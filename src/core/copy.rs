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

        preprocess_directory(source, destination, options)?
    } else {
        preprocess_file(
            source,
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
        options.style.apply(&pb, plan.total_files);
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
                *options,
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
                        *options,
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
        if matches!(options.style, ProgressBarStyle::Detailed) && !options.attributes_only {
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
    options: CopyOptions,
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
                    update_progress(overall_pb, completed_files, total_files, &options);
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
        update_progress(overall_pb, completed_files, total_files, &options);
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

    const MAX_UPDATES: u64 = 16;
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

    update_progress(overall_pb, completed_files, total_files, &options);

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
        && matches!(options.style, ProgressBarStyle::Detailed)
    {
        pb.set_message(format!("Copying: {}/{} files", completed, total_files));
    }
}
