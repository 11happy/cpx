use crate::cli::args::{BackupMode, CopyOptions, FollowSymlink};
#[cfg(target_os = "linux")]
use crate::core::fast_copy::fast_copy;
use crate::utility::backup::{create_backup, generate_backup_path};
use crate::utility::helper::{
    create_directories, create_hardlink, create_symlink, prompt_overwrite,
};
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{path::Path, path::PathBuf};

pub fn copy(source: &Path, destination: &Path, options: &CopyOptions) -> io::Result<()> {
    let source_metadata = match options.follow_symlink {
        FollowSymlink::Dereference | FollowSymlink::CommandLineSymlink => {
            std::fs::metadata(source)?
        }
        FollowSymlink::NoDereference => std::fs::symlink_metadata(source)?,
    };
    let source_root = source.parent().unwrap_or(source);
    let destination_metadata = std::fs::metadata(destination).ok();

    let plan = if source_metadata.is_dir() {
        if !options.recursive {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "'{}' is a directory (not copied, use -r to copy recursively)",
                    source.display()
                ),
            ));
        }

        if let Some(dest_meta) = destination_metadata
            && dest_meta.is_file()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("'{}' is a file, expected directory", destination.display()),
            ));
        }

        preprocess_directory(source, source_root, destination, options)?
    } else {
        preprocess_file(
            source,
            source_root,
            destination,
            options,
            source_metadata,
            destination_metadata,
        )?
    };

    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }

    execute_copy(plan, options)
}

pub fn multiple_copy(
    sources: Vec<PathBuf>,
    destination: PathBuf,
    options: &CopyOptions,
) -> io::Result<()> {
    let plan = preprocess_multiple(&sources, &destination, options)?;
    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }
    execute_copy(plan, options)
}

fn execute_copy(plan: CopyPlan, options: &CopyOptions) -> io::Result<()> {
    if !options.attributes_only {
        create_directories(&plan.directories)?;
    } else {
        for dir_task in &plan.directories {
            if let Some(src) = &dir_task.source
                && std::fs::symlink_metadata(&dir_task.destination).is_ok()
            {
                preserve::apply_preserve_attrs(src, &dir_task.destination, options.preserve)?;
            }
        }
    }

    if options.hard_link {
        for hardlink_task in &plan.hardlinks {
            if let Err(e) = create_hardlink(hardlink_task, options) {
                eprintln!(
                    "Failed to create hardlink {:?} -> {:?}: {}",
                    hardlink_task.destination, hardlink_task.source, e
                );
                return Err(e);
            }
        }

        if plan.total_hardlinks > 0 {
            println!("Created {} hard links", plan.total_hardlinks);
        }
        return Ok(());
    }

    if options.symbolic_link.is_some() {
        for symlink_task in &plan.symlinks {
            if let Err(e) = create_symlink(symlink_task) {
                eprintln!(
                    "Failed to create symlink {:?} -> {:?}: {}",
                    symlink_task.destination, symlink_task.source, e
                );
                return Err(e);
            }
        }

        if plan.total_symlinks > 0 {
            println!("Created {} symbolic links", plan.total_symlinks);
        }
        return Ok(());
    }

    let overall_pb = if plan.total_files >= 1 && !options.interactive && !options.attributes_only {
        let pb = ProgressBar::new(plan.total_size);
        options.progress_bar.apply(&pb, plan.total_files);
        Some(Arc::new(pb))
    } else {
        None
    };

    let completed_files = Arc::new(AtomicUsize::new(0));

    // For interactive mode, process sequentially
    if options.interactive {
        for file_task in plan.files {
            copy_core(
                &file_task.source,
                &file_task.destination,
                file_task.size,
                overall_pb.as_deref(),
                &completed_files,
                plan.total_files,
                options,
            )?;
        }
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(options.concurrency)
            .build()
            .map_err(|e| io::Error::other(format!("Failed to create thread pool: {}", e)))?;

        let results: Vec<_> = pool.install(|| {
            plan.files
                .par_iter()
                .map(|file_task| {
                    copy_core(
                        &file_task.source,
                        &file_task.destination,
                        file_task.size,
                        overall_pb.as_deref(),
                        &completed_files,
                        plan.total_files,
                        options,
                    )
                })
                .collect()
        });

        let errors: Vec<_> = results
            .into_iter()
            .enumerate()
            .filter_map(|(i, r)| r.err().map(|e| format!("File {}: {}", i, e)))
            .collect();

        if !errors.is_empty() {
            if let Some(pb) = overall_pb {
                pb.abandon_with_message("Copy completed with errors");
            }
            return Err(io::Error::other(format!(
                "Errors occurred:\n{}",
                errors.join("\n")
            )));
        }
    }

    if let Some(pb) = overall_pb {
        if matches!(options.progress_bar.style, ProgressBarStyle::Detailed)
            && !options.attributes_only
        {
            pb.finish_with_message(format!("Copied {} files successfully", plan.total_files));
        } else {
            pb.finish_with_message("Done".to_string());
        }
    }

    Ok(())
}

fn copy_core(
    source: &Path,
    destination: &Path,
    file_size: u64,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
) -> io::Result<()> {
    if options.attributes_only {
        if std::fs::symlink_metadata(destination).is_err() {
            return Ok(());
        }
        preserve::apply_preserve_attrs(source, destination, options.preserve)?;
        return Ok(());
    }

    if options.interactive
        && destination.try_exists().unwrap_or(false)
        && !prompt_overwrite(destination)?
    {
        return Ok(());
    }

    if let Some(backup_mode) = options.backup
        && backup_mode != BackupMode::None
        && destination.try_exists().unwrap_or(false)
    {
        let backup_path = generate_backup_path(destination, backup_mode)?;
        let _ = create_backup(destination, &backup_path);
    }

    if options.remove_destination {
        let _ = std::fs::remove_file(destination);
    }

    if let Some(reflink_mode) = options.reflink {
        use crate::cli::args::ReflinkMode;
        if reflink_mode != ReflinkMode::Never {
            if destination.try_exists().unwrap_or(false) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "cannot reflink '{}' to '{}': destination exists",
                        source.display(),
                        destination.display()
                    ),
                ));
            }

            match reflink_copy::reflink(source, destination) {
                Ok(()) => {
                    if let Some(pb) = overall_pb {
                        pb.inc(file_size);
                    }
                    update_progress(overall_pb, completed_files, total_files, options);
                    if options.preserve != PreserveAttr::none() {
                        preserve::apply_preserve_attrs(source, destination, options.preserve)?;
                    }
                    return Ok(());
                }
                Err(e) if reflink_mode == ReflinkMode::Always => {
                    return Err(io::Error::new(io::ErrorKind::Unsupported, e));
                }
                Err(_) => {}
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Ok(true) = fast_copy(source, destination, file_size, overall_pb, options) {
        update_progress(overall_pb, completed_files, total_files, options);
        if options.preserve != PreserveAttr::none() {
            preserve::apply_preserve_attrs(source, destination, options.preserve)?;
        }
        return Ok(());
    }

    let mut src_file = std::fs::File::open(source)?;
    let dest_file = match std::fs::File::create(destination) {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = std::fs::remove_file(destination);
            std::fs::File::create(destination)?
        }
        Err(e) => return Err(e),
    };

    let buffer_size: usize = if file_size < 1024 * 1024 {
        64 * 1024
    } else if file_size < 8 * 1024 * 1024 {
        256 * 1024
    } else if file_size < 64 * 1024 * 1024 {
        512 * 1024
    } else if file_size < 512 * 1024 * 1024 {
        1024 * 1024
    } else {
        2 * 1024 * 1024
    };

    let mut dest_file = std::io::BufWriter::with_capacity(buffer_size, dest_file);
    let mut buffer = vec![0u8; buffer_size];

    const MAX_UPDATES: u64 = 128;
    let update_threshold = if file_size > MAX_UPDATES * buffer_size as u64 {
        file_size / MAX_UPDATES
    } else {
        buffer_size as u64
    };

    let mut accumulated_bytes = 0u64;

    loop {
        let bytes_read = src_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest_file.write_all(&buffer[..bytes_read])?;

        accumulated_bytes += bytes_read as u64;
        if accumulated_bytes >= update_threshold {
            if let Some(pb) = overall_pb {
                pb.inc(accumulated_bytes);
            }
            accumulated_bytes = 0;
        }
    }

    if accumulated_bytes > 0
        && let Some(pb) = overall_pb
    {
        pb.inc(accumulated_bytes);
    }

    dest_file.flush()?;

    update_progress(overall_pb, completed_files, total_files, options);

    if options.preserve != PreserveAttr::none() {
        preserve::apply_preserve_attrs(source, destination, options.preserve)?;
    }

    Ok(())
}

fn update_progress(
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
) {
    let completed = completed_files.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(pb) = overall_pb
        && matches!(options.progress_bar.style, ProgressBarStyle::Detailed)
    {
        pb.set_message(format!("Copying: {}/{} files", completed, total_files));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utility::progress_bar::ProgressOptions;
    use std::fs;
    use tempfile::TempDir;
    fn default_copy_options() -> CopyOptions {
        CopyOptions {
            recursive: false,
            resume: false,
            force: false,
            interactive: false,
            preserve: PreserveAttr::none(),
            backup: None,
            symbolic_link: None,
            hard_link: false,
            follow_symlink: FollowSymlink::NoDereference,
            attributes_only: false,
            remove_destination: false,
            reflink: None,
            parents: false,
            concurrency: 1,
            exclude_rules: None,
            progress_bar: ProgressOptions::default(),
        }
    }

    #[test]
    fn test_copy_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test content").unwrap();

        let options = default_copy_options();
        copy(&source, &dest, &options).unwrap();

        assert!(dest.exists());
        let content = fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_copy_directory_without_recursive_fails() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_dir = temp_dir.path().join("dest_dir");

        fs::create_dir(&source_dir).unwrap();

        let options = default_copy_options();
        let result = copy(&source_dir, &dest_dir, &options);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("use -r"));
    }

    #[test]
    fn test_copy_directory_with_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_dir = temp_dir.path().join("dest_dir");

        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("file.txt"), b"content").unwrap();
        fs::create_dir(&dest_dir).unwrap();

        let mut options = default_copy_options();
        options.recursive = true;

        copy(&source_dir, &dest_dir, &options).unwrap();

        assert!(dest_dir.exists());
        assert!(dest_dir.join("source_dir").join("file.txt").exists());
        let content = fs::read_to_string(dest_dir.join("source_dir").join("file.txt")).unwrap();
        assert_eq!(content, "content");
    }

    #[test]
    fn test_copy_with_force_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"new content").unwrap();
        fs::write(&dest, b"old content").unwrap();

        let mut options = default_copy_options();
        options.force = true;

        copy(&source, &dest, &options).unwrap();

        let content = fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_copy_preserves_timestamps() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test").unwrap();

        let mut options = default_copy_options();
        options.preserve.timestamps = true;

        copy(&source, &dest, &options).unwrap();

        let src_mtime = fs::metadata(&source).unwrap().modified().unwrap();
        let dest_mtime = fs::metadata(&dest).unwrap().modified().unwrap();

        let diff = if src_mtime > dest_mtime {
            src_mtime.duration_since(dest_mtime).unwrap()
        } else {
            dest_mtime.duration_since(src_mtime).unwrap()
        };

        assert!(diff.as_secs() < 1);
    }

    #[test]
    fn test_multiple_copy() {
        let temp_dir = TempDir::new().unwrap();
        let source1 = temp_dir.path().join("source1.txt");
        let source2 = temp_dir.path().join("source2.txt");
        let dest_dir = temp_dir.path().join("dest");

        fs::write(&source1, b"content1").unwrap();
        fs::write(&source2, b"content2").unwrap();
        fs::create_dir(&dest_dir).unwrap();

        let sources = vec![source1.clone(), source2.clone()];
        let options = default_copy_options();

        multiple_copy(sources, dest_dir.clone(), &options).unwrap();

        assert!(dest_dir.join("source1.txt").exists());
        assert!(dest_dir.join("source2.txt").exists());
    }

    #[test]
    fn test_copy_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("empty.txt");
        let dest = temp_dir.path().join("empty_copy.txt");

        fs::write(&source, b"").unwrap();

        let options = default_copy_options();
        copy(&source, &dest, &options).unwrap();

        assert!(dest.exists());
        let content = fs::read(&dest).unwrap();
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_copy_large_buffer_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("large.txt");
        let dest = temp_dir.path().join("large_copy.txt");

        // Create a file larger than 64MB to test buffer size calculation
        let content = vec![b'x'; 70 * 1024 * 1024]; // 70MB
        fs::write(&source, content).unwrap();

        let options = default_copy_options();
        copy(&source, &dest, &options).unwrap();

        assert!(dest.exists());
        assert_eq!(fs::metadata(&dest).unwrap().len(), 70 * 1024 * 1024);
    }
}
