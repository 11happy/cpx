use crate::cli::args::{BackupMode, CopyOptions, FollowSymlink};
#[cfg(target_os = "linux")]
use crate::core::fast_copy::fast_copy;
use crate::error::{CopyError, CopyResult};
use crate::utility::backup::{create_backup, generate_backup_path};
use crate::utility::helper::{
    create_directories, create_hardlink, create_symlink, move_file, move_hardlink, move_symlink,
    prompt_deletion, prompt_overwrite, update_progress,
};
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, HardLinkTracker, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use indicatif::ProgressBar;
use rayon::prelude::*;

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{path::Path, path::PathBuf};

pub fn copy(source: &Path, destination: &Path, options: &CopyOptions) -> CopyResult<()> {
    let source_metadata = match options.follow_symlink {
        FollowSymlink::Dereference | FollowSymlink::CommandLineSymlink => std::fs::metadata(source)
            .map_err(|_e| CopyError::InvalidSource(source.to_path_buf()))?,
        FollowSymlink::NoDereference => std::fs::symlink_metadata(source)
            .map_err(|_e| CopyError::InvalidSource(source.to_path_buf()))?,
    };
    let source_root = source.parent().unwrap_or(source);
    let destination_metadata = std::fs::metadata(destination).ok();

    let mut plan = if source_metadata.is_dir() {
        if !options.recursive {
            return Err(CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: "'src' is a directory (not copied, use -r to copy recursively)".to_string(),
            });
        }

        if let Some(dest_meta) = destination_metadata
            && dest_meta.is_file()
        {
            return Err(CopyError::InvalidDestination(destination.to_path_buf()));
        }

        preprocess_directory(source, source_root, destination, options).map_err(|e| {
            CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: e.to_string(),
            }
        })?
    } else {
        preprocess_file(
            source,
            source_root,
            destination,
            options,
            source_metadata,
            destination_metadata,
        )
        .map_err(|e| CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: e.to_string(),
        })?
    };

    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }

    plan.source = Some(source.to_path_buf());
    execute_copy(plan, options)
}

pub fn multiple_copy(
    sources: Vec<PathBuf>,
    destination: PathBuf,
    options: &CopyOptions,
) -> CopyResult<()> {
    let mut plan = preprocess_multiple(&sources, &destination, options).map_err(|e| {
        CopyError::CopyFailed {
            source: sources[0].clone(),
            destination: destination.clone(),
            reason: e.to_string(),
        }
    })?;
    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }
    plan.source = Some(sources[0].clone());
    execute_copy(plan, options)
}

fn execute_copy(plan: CopyPlan, options: &CopyOptions) -> CopyResult<()> {
    if !options.attributes_only {
        create_directories(&plan.directories)?;
    } else {
        for dir_task in &plan.directories {
            if let Some(src) = &dir_task.source
                && std::fs::symlink_metadata(&dir_task.destination).is_ok()
            {
                preserve::apply_preserve_attrs(src, &dir_task.destination, options.preserve)
                    .map_err(|e| CopyError::CopyFailed {
                        source: src.clone(),
                        destination: dir_task.destination.clone(),
                        reason: e.to_string(),
                    })?;
            }
        }
    }

    if options.hard_link {
        for hardlink_task in &plan.hardlinks {
            create_hardlink(hardlink_task, options)?;
        }

        if plan.total_hardlinks > 0 {
            println!("Created {} hard links", plan.total_hardlinks);
        }
        return Ok(());
    }

    if !plan.symlinks.is_empty() {
        for symlink_task in &plan.symlinks {
            if options.move_files {
                move_symlink(symlink_task, options)?;
            } else {
                create_symlink(symlink_task).map_err(|_e| CopyError::SymlinkFailed {
                    source: symlink_task.source.clone(),
                    destination: symlink_task.destination.clone(),
                })?;
            }
        }
        if plan.total_symlinks > 0 {
            if options.move_files {
                println!("Moved {} symbolic links", plan.total_symlinks);
            } else {
                println!("Created {} symbolic links", plan.total_symlinks);
            }
        }

        if options.symbolic_link.is_some() {
            return Ok(());
        }
    }

    if options.move_files && !plan.hardlinks.is_empty() {
        for hardlink_task in &plan.hardlinks {
            move_hardlink(hardlink_task, options)?;
        }
        if plan.total_hardlinks > 0 {
            println!("Moved {} hard links", plan.total_hardlinks);
        }
    }

    let overall_pb = if plan.total_files >= 1 && !options.interactive && !options.attributes_only {
        let pb = ProgressBar::new(plan.total_size);
        options.progress_bar.apply(&pb, plan.total_files);
        Some(Arc::new(pb))
    } else {
        None
    };

    let completed_files = Arc::new(AtomicUsize::new(0));

    // Initialize hard link tracker if preserve.links is enabled
    let hardlink_tracker = if options.preserve.links {
        Some(Arc::new(Mutex::new(HardLinkTracker::new())))
    } else {
        None
    };

    // For interactive mode, process sequentially
    if options.interactive {
        // Prompt once for move_files confirmation
        if options.move_files
            && let Some(ref source) = plan.source
            && !prompt_deletion(source)?
        {
            return Ok(());
        }
        for file_task in &plan.files {
            if options.move_files {
                move_file(
                    &file_task.source,
                    &file_task.destination,
                    overall_pb.as_deref(),
                    &completed_files,
                    plan.total_files,
                    options,
                )?;
            } else {
                copy_core(
                    &file_task.source,
                    &file_task.destination,
                    file_task.size,
                    overall_pb.as_deref(),
                    &completed_files,
                    plan.total_files,
                    options,
                    hardlink_tracker.as_ref(),
                )?;
            }
        }
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(options.parallel)
            .build()
            .map_err(|e| CopyError::CopyFailed {
                source: PathBuf::new(),
                destination: PathBuf::new(),
                reason: format!("Failed to create thread pool: {}", e),
            })?;

        let results: Vec<_> = pool.install(|| {
            plan.files
                .par_iter()
                .map(|file_task| {
                    let result = if options.move_files {
                        move_file(
                            &file_task.source,
                            &file_task.destination,
                            overall_pb.as_deref(),
                            &completed_files,
                            plan.total_files,
                            options,
                        )
                    } else {
                        copy_core(
                            &file_task.source,
                            &file_task.destination,
                            file_task.size,
                            overall_pb.as_deref(),
                            &completed_files,
                            plan.total_files,
                            options,
                            hardlink_tracker.as_ref(),
                        )
                    };

                    match result {
                        Ok(()) => Ok(()),
                        Err(e) => Err((file_task.source.clone(), file_task.destination.clone(), e)),
                    }
                })
                .collect()
        });

        let mut interrupted = false;
        let mut errors: Vec<(PathBuf, PathBuf, CopyError)> = Vec::new();

        for result in results.into_iter() {
            if let Err((source, dest, e)) = result {
                match e {
                    CopyError::Io(ref io_err) if io_err.kind() == io::ErrorKind::Interrupted => {
                        interrupted = true;
                    }
                    _ => {
                        errors.push((source, dest, e));
                    }
                }
            }
        }

        if interrupted {
            let completed = completed_files.load(Ordering::Relaxed);

            eprintln!("\nCompleted:  {} files", completed);
            eprintln!("Remaining:  {} files", plan.total_files - completed);

            return Err(CopyError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation interrupted by user",
            )));
        }

        if !errors.is_empty() {
            if let Some(pb) = overall_pb {
                pb.abandon_with_message("Completed with errors");
            }
            eprintln!("\nFailed to copy {} file(s):", errors.len());
            for (source, _dest, err) in errors.iter().take(3) {
                eprintln!("  {} - {}", source.display(), err);
            }
            if errors.len() > 3 {
                eprintln!("  ... and {} more", errors.len() - 5);
            }
            return Err(CopyError::Io(io::Error::other(format!(
                "{} file(s) failed to copy",
                errors.len()
            ))));
        }
    }

    // Clean up empty source directories after moving files
    if options.move_files {
        cleanup_empty_directories(&plan.directories);
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

#[allow(clippy::too_many_arguments)]
fn copy_core(
    source: &Path,
    destination: &Path,
    file_size: u64,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
    hardlink_tracker: Option<&Arc<Mutex<HardLinkTracker>>>,
) -> CopyResult<()> {
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

    // Handle hard link preservation
    if let Some(tracker) = hardlink_tracker {
        let mut tracker_guard = tracker.lock().map_err(|_| {
            CopyError::Io(io::Error::other("Failed to acquire hardlink tracker lock"))
        })?;

        if tracker_guard.track_and_create_link(source, destination)? {
            // Hard link was created, no need to copy file content
            update_progress(overall_pb, completed_files, total_files, options);
            if options.preserve != PreserveAttr::none() {
                preserve::apply_preserve_attrs(source, destination, options.preserve)
                    .map_err(CopyError::from)?;
            }

            return Ok(());
        }
        // Continue with normal file copy if this is the first file in the inode group
    }

    if let Some(reflink_mode) = options.reflink {
        use crate::cli::args::ReflinkMode;
        if reflink_mode != ReflinkMode::Never {
            if destination.try_exists().unwrap_or(false) {
                return Err(CopyError::ReflinkFailed {
                    source: source.to_path_buf(),
                    destination: destination.to_path_buf(),
                });
            }

            match reflink_copy::reflink(source, destination) {
                Ok(()) => {
                    if let Some(pb) = overall_pb {
                        pb.inc(file_size);
                    }
                    update_progress(overall_pb, completed_files, total_files, options);
                    if options.preserve != PreserveAttr::none() {
                        preserve::apply_preserve_attrs(source, destination, options.preserve)
                            .map_err(CopyError::from)?;
                    }

                    return Ok(());
                }
                Err(e) if reflink_mode == ReflinkMode::Always => {
                    return Err(CopyError::ReflinkFailed {
                        source: source.to_path_buf(),
                        destination: destination.to_path_buf(),
                    });
                }
                Err(_) => {}
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if options.abort.load(Ordering::Relaxed) {
            return Err(CopyError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted by user",
            )));
        }
        if let Ok(true) = fast_copy(source, destination, file_size, overall_pb, options) {
            update_progress(overall_pb, completed_files, total_files, options);
            if options.preserve != PreserveAttr::none() {
                preserve::apply_preserve_attrs(source, destination, options.preserve)
                    .map_err(CopyError::from)?;
            }

            return Ok(());
        }
    }

    // Copy file in chunks
    let mut src_file = std::fs::File::open(source)?;
    let dest_file = match std::fs::File::create(destination) {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = std::fs::remove_file(destination);
            std::fs::File::create(destination)?
        }
        Err(e) => return Err(CopyError::Io(e)),
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
        if options.abort.load(Ordering::Relaxed) {
            dest_file.flush()?;
            drop(dest_file);
            if let Err(e) = std::fs::remove_file(destination) {
                eprintln!(
                    "Could not remove incomplete file {}: {}",
                    destination.display(),
                    e
                );
            } else {
                eprintln!("Cleaned up incomplete file: {}", destination.display());
            }

            return Err(CopyError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted by user",
            )));
        }

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
        preserve::apply_preserve_attrs(source, destination, options.preserve)
            .map_err(CopyError::from)?;
    }

    Ok(())
}

// Clean up empty source directories after moving files.
fn cleanup_empty_directories(directories: &[crate::utility::preprocess::DirectoryTask]) {
    let mut dirs_to_clean: Vec<PathBuf> = directories
        .iter()
        .filter_map(|task| task.source.clone())
        .collect();

    dirs_to_clean.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for dir in dirs_to_clean {
        match std::fs::remove_dir(&dir) {
            Ok(()) => {}
            Err(e) => {
                eprintln!(
                    "Warning: Failed to remove directory '{}': {}",
                    dir.display(),
                    e
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utility::progress_bar::ProgressOptions;
    use std::fs;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;
    fn default_copy_options() -> CopyOptions {
        CopyOptions {
            recursive: false,
            move_files: false,
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
            parallel: 1,
            exclude_rules: None,
            progress_bar: ProgressOptions::default(),
            abort: Arc::new(AtomicBool::new(false)),
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

    #[test]
    fn test_move_file_to_different_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_dir = temp_dir.path().join("dest_dir");
        let source_file = source_dir.join("file.txt");
        let dest_file = dest_dir.join("file.txt");

        fs::create_dir(&source_dir).unwrap();
        fs::create_dir(&dest_dir).unwrap();
        fs::write(&source_file, b"moved to different directory").unwrap();

        let mut options = default_copy_options();
        options.move_files = true;

        copy(&source_file, &dest_file, &options).unwrap();

        assert!(dest_file.exists());
        let content = fs::read_to_string(&dest_file).unwrap();
        assert_eq!(content, "moved to different directory");

        assert!(!source_file.exists());
    }

    #[test]
    fn test_copy_with_move_files() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test content for delete original").unwrap();

        let mut options = default_copy_options();
        options.move_files = true;

        copy(&source, &dest, &options).unwrap();
        assert!(dest.exists());
        let content = fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "test content for delete original");

        assert!(!source.exists());
    }

    #[test]
    fn test_multiple_copy_with_move_files() {
        let temp_dir = TempDir::new().unwrap();
        let source1 = temp_dir.path().join("source1.txt");
        let source2 = temp_dir.path().join("source2.txt");
        let dest_dir = temp_dir.path().join("dest");

        fs::write(&source1, b"content1").unwrap();
        fs::write(&source2, b"content2").unwrap();
        fs::create_dir(&dest_dir).unwrap();

        let sources = vec![source1.clone(), source2.clone()];
        let mut options = default_copy_options();
        options.move_files = true;

        multiple_copy(sources, dest_dir.clone(), &options).unwrap();

        assert!(dest_dir.join("source1.txt").exists());
        assert!(dest_dir.join("source2.txt").exists());
        assert_eq!(
            fs::read_to_string(dest_dir.join("source1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(dest_dir.join("source2.txt")).unwrap(),
            "content2"
        );

        assert!(!source1.exists());
        assert!(!source2.exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_move_symlink() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let source_link = temp_dir.path().join("source_link");
        let dest_link = temp_dir.path().join("dest_link");

        fs::write(&target, "symlink target content").unwrap();
        symlink(&target, &source_link).unwrap();

        let mut options = default_copy_options();
        options.move_files = true;

        copy(&source_link, &dest_link, &options).unwrap();

        // Destination symlink should exist and point to target
        // Note: exists() follows symlinks, so use symlink_metadata to check if symlink exists
        assert!(fs::symlink_metadata(&dest_link).is_ok());
        assert_eq!(fs::read_link(&dest_link).unwrap(), target);

        // Source symlink should be removed
        assert!(!source_link.exists());

        // Target file should still exist
        assert!(target.exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_move_hardlink() {
        use std::fs::hard_link;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let hardlink = temp_dir.path().join("hardlink.txt");
        let dest_dir = temp_dir.path().join("dest");
        let dest_file = dest_dir.join("source.txt");

        fs::create_dir(&dest_dir).unwrap();
        fs::write(&source, "hardlink content").unwrap();
        hard_link(&source, &hardlink).unwrap();

        let mut options = default_copy_options();
        options.move_files = true;

        copy(&source, &dest_file, &options).unwrap();

        // Destination file should exist with correct content
        assert!(dest_file.exists());
        assert_eq!(fs::read_to_string(&dest_file).unwrap(), "hardlink content");

        // Source file should be removed
        assert!(!source.exists());

        // Original hardlink should still exist and have same inode
        assert!(hardlink.exists());
        assert_eq!(fs::read_to_string(&hardlink).unwrap(), "hardlink content");
    }
}
