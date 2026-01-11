use crate::cli::args::CopyOptions;
use crate::utility::helper::prompt_overwrite;
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use indicatif::ProgressBar;
use std::io::{self};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{path::Path, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Semaphore;

pub async fn copy(
    source: &Path,
    destination: &Path,
    style: ProgressBarStyle,
    options: &CopyOptions,
) -> io::Result<()> {
    let src_metadata = tokio::fs::metadata(source).await?;

    let plan = if src_metadata.is_dir() {
        if !options.recursive {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "'{}' is a directory (not copied, use -r to copy recursively)",
                    source.display()
                ),
            ));
        }

        if let Ok(dest_meta) = tokio::fs::metadata(destination).await {
            if dest_meta.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("'{}' is a file, expected directory", destination.display()),
                ));
            }
        }

        preprocess_directory(source, destination, options.resume, options.parents).await?
    } else {
        preprocess_file(source, destination, options.resume, options.parents).await?
    };
    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }

    execute_copy(plan, style, options).await
}

pub async fn multiple_copy(
    sources: Vec<PathBuf>,
    destination: PathBuf,
    style: ProgressBarStyle,
    options: &CopyOptions,
) -> io::Result<()> {
    let plan = preprocess_multiple(&sources, &destination, options.resume, options.parents).await?;
    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }
    execute_copy(plan, style, options).await
}

async fn execute_copy(
    plan: CopyPlan,
    style: ProgressBarStyle,
    options: &CopyOptions,
) -> io::Result<()> {
    if !options.attributes_only {
        for dir in &plan.directories {
            if let Err(e) = tokio::fs::create_dir_all(&dir.destination).await {
                if e.kind() != io::ErrorKind::AlreadyExists {
                    return Err(e);
                }
            }
        }
    } else {
        for dir_task in &plan.directories {
            if let Some(src) = &dir_task.source {
                if tokio::fs::symlink_metadata(&dir_task.destination)
                    .await
                    .is_ok()
                {
                    preserve::apply_preserve_attrs(src, &dir_task.destination, options.preserve)
                        .await?;
                }
            }
        }
    }

    let overall_pb = if plan.total_files >= 1 && !options.interactive && !options.attributes_only {
        let pb = ProgressBar::new(plan.total_size);
        style.apply(&pb, plan.total_files);
        Some(Arc::new(pb))
    } else {
        None
    };
    let concurrency = if options.interactive {
        1
    } else {
        options.concurrency
    };
    let completed_files = Arc::new(AtomicUsize::new(0));
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut tasks = Vec::new();

    for file_task in plan.files {
        let sem = semaphore.clone();
        let overall = overall_pb.clone();
        let style_cloned = style;
        let options_copy = *options;
        let completed = completed_files.clone();
        let total_files = plan.total_files;

        let task = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Semaphore closed"))?;

            let result = copy_core(
                &file_task.source,
                &file_task.destination,
                file_task.size,
                style_cloned,
                overall.as_deref(),
                completed.as_ref(),
                total_files,
                options_copy,
            )
            .await;

            result
        });

        tasks.push(task);
    }

    let mut errors = Vec::new();
    for (i, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(format!("File {}: {}", i, e)),
            Err(e) => errors.push(format!("Task {}: {}", i, e)),
        }
    }

    if let Some(pb) = overall_pb {
        if errors.is_empty() {
            if matches!(style, ProgressBarStyle::Default) && !options.attributes_only {
                pb.finish_with_message(format!("Copied {} files successfully", plan.total_files)); // TODO: see a good message
            } else {
                pb.finish_with_message(format!("Done")); // TODO: see a good message
            }
        } else {
            pb.abandon_with_message("Copy completed with errors");
        }
    }

    if !errors.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Errors occurred:\n{}", errors.join("\n")),
        ));
    }

    Ok(())
}

async fn copy_core(
    source: &Path,
    destination: &Path,
    file_size: u64,
    style: ProgressBarStyle,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: CopyOptions,
) -> io::Result<()> {
    if options.attributes_only {
        if tokio::fs::symlink_metadata(destination).await.is_err() {
            return Ok(());
        }
        preserve::apply_preserve_attrs(source, destination, options.preserve).await?;
        return Ok(());
    }
    let src_file = tokio::fs::File::open(source).await?;

    if options.interactive && tokio::fs::metadata(destination).await.is_ok() {
        if !prompt_overwrite(destination)? {
            return Ok(());
        }
    }
    let dest_file = match tokio::fs::File::create(destination).await {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = tokio::fs::remove_file(destination).await;
            tokio::fs::File::create(destination).await?
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

    let mut src_file = tokio::io::BufReader::with_capacity(buffer_size, src_file);
    let mut dest_file = tokio::io::BufWriter::with_capacity(buffer_size, dest_file);

    let mut buffer = vec![0u8; buffer_size];

    const MAX_UPDATES: u64 = 128;
    let update_threshold = if file_size > MAX_UPDATES * buffer_size as u64 {
        file_size / MAX_UPDATES
    } else {
        buffer_size as u64
    };

    let mut accumulated_bytes = 0u64;

    loop {
        let bytes_read = src_file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        dest_file.write_all(&buffer[..bytes_read]).await?;

        accumulated_bytes += bytes_read as u64;
        if accumulated_bytes >= update_threshold {
            if let Some(pb) = overall_pb {
                pb.inc(accumulated_bytes);
            }
            accumulated_bytes = 0;
        }
    }
    if accumulated_bytes > 0 {
        if let Some(pb) = overall_pb {
            pb.inc(accumulated_bytes);
        }
    }
    dest_file.flush().await?;
    let completed = completed_files.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(pb) = overall_pb
        && matches!(style, ProgressBarStyle::Default)
    {
        pb.set_message(format!("Copying: {}/{} files", completed, total_files));
    }
    if options.preserve != PreserveAttr::none() {
        preserve::apply_preserve_attrs(source, destination, options.preserve).await?;
    }
    Ok(())
}
