use crate::cli::args::CopyOptions;
use crate::utility::helper::prompt_overwrite;
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use indicatif::{MultiProgress, ProgressBar};
use std::io::{self};
use std::sync::Arc;
use std::{path::Path, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
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
    for dir in &plan.directories {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        }
    }

    let multi_progress = MultiProgress::new();
    let overall_pb = if plan.total_files >= 1 && !options.interactive {
        let pb = multi_progress.add(ProgressBar::new(plan.total_size));
        pb.set_message(format!("Copying {} files", plan.total_files));
        style.apply(&pb);
        Some(pb)
    } else {
        None
    };
    let concurrency = if options.interactive {
        1
    } else {
        options.concurrency
    };
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut tasks = Vec::new();

    for file_task in plan.files {
        let sem = semaphore.clone();
        let mp = multi_progress.clone();
        let overall = overall_pb.clone();
        let style_cloned = style;
        let options_copy = *options;

        let task = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Semaphore closed"))?;

            let pb = if options_copy.interactive {
                ProgressBar::hidden()
            } else {
                let pb = mp.add(ProgressBar::new(file_task.size));
                let file_name = file_task
                    .source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                pb.set_message(format!("{}", file_name));
                style_cloned.apply(&pb);
                pb
            };

            let result = copy_core(
                &file_task.source,
                &file_task.destination,
                file_task.size,
                &pb,
                overall.as_ref(),
                options_copy,
            )
            .await;

            match &result {
                Ok(_) => pb.finish_and_clear(),
                Err(_) => pb.abandon_with_message("Copy failed"),
            }

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
            pb.finish_with_message(format!("Copied {} files successfully", plan.total_files));
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
    file_pb: &ProgressBar,
    overall_pb: Option<&ProgressBar>,
    options: CopyOptions,
) -> io::Result<()> {
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

    let mut src_file = BufReader::new(src_file);
    let mut dest_file = BufWriter::new(dest_file);

    const BUFFER_SIZE: usize = 512 * 1024;
    let mut buffer = vec![0u8; BUFFER_SIZE];

    const MAX_UPDATES: u64 = 200;
    let update_threshold = if file_size > MAX_UPDATES * BUFFER_SIZE as u64 {
        file_size / MAX_UPDATES
    } else {
        BUFFER_SIZE as u64
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
            file_pb.inc(accumulated_bytes);
            if let Some(pb) = overall_pb {
                pb.inc(accumulated_bytes);
            }
            accumulated_bytes = 0;
        }
    }
    if accumulated_bytes > 0 {
        file_pb.inc(accumulated_bytes);
        if let Some(pb) = overall_pb {
            pb.inc(accumulated_bytes);
        }
    }
    dest_file.flush().await?;
    if options.preserve != PreserveAttr::none() {
        preserve::apply_preserve_attrs(source, destination, options.preserve).await?;
    }
    Ok(())
}
