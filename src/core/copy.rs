use crate::cli::args::CopyOptions;
#[cfg(target_os = "linux")]
use crate::core::fast_copy::fast_copy;
use crate::utility::helper::{
    create_directories, create_hardlink, create_symlink, prompt_overwrite,
};
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use futures::stream::{FuturesUnordered, StreamExt};
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
    let source_metadata = tokio::fs::symlink_metadata(source).await?;
    let destination_metadata = tokio::fs::metadata(destination).await.ok();
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

    execute_copy(plan, style, options).await
}

pub async fn multiple_copy(
    sources: Vec<PathBuf>,
    destination: PathBuf,
    style: ProgressBarStyle,
    options: &CopyOptions,
) -> io::Result<()> {
    let plan = preprocess_multiple(&sources, &destination, options)?;
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
        create_directories(&plan.directories)?;
    } else {
        for dir_task in &plan.directories {
            if let Some(src) = &dir_task.source
                && tokio::fs::symlink_metadata(&dir_task.destination)
                    .await
                    .is_ok()
            {
                preserve::apply_preserve_attrs(src, &dir_task.destination, options.preserve)
                    .await?;
            }
        }
    }
    if options.hard_link {
        for hardlink_task in &plan.hardlinks {
            if let Err(e) = create_hardlink(hardlink_task, options).await {
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
            if let Err(e) = create_symlink(symlink_task).await {
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
    let mut tasks = FuturesUnordered::new();

    for file_task in plan.files {
        let sem = semaphore.clone();
        let overall = overall_pb.clone();
        let style_cloned = style;
        let options_copy = *options;
        let completed = completed_files.clone();
        let total_files = plan.total_files;

        tasks.push(tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| io::Error::other("Semaphore closed"))?;

            copy_core(
                &file_task.source,
                &file_task.destination,
                file_task.size,
                style_cloned,
                overall.as_deref(),
                completed.as_ref(),
                total_files,
                options_copy,
            )
            .await
        }));
    }

    let mut errors = Vec::new();
    let mut index = 0;
    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(format!("File {}: {}", index, e)),
            Err(e) => errors.push(format!("Task {}: {}", index, e)),
        }
        index += 1;
    }

    if let Some(pb) = overall_pb {
        if errors.is_empty() {
            if matches!(style, ProgressBarStyle::Default) && !options.attributes_only {
                pb.finish_with_message(format!("Copied {} files successfully", plan.total_files)); // TODO: see a good message
            } else {
                pb.finish_with_message("Done".to_string()); // TODO: see a good message
            }
        } else {
            pb.abandon_with_message("Copy completed with errors");
        }
    }

    if !errors.is_empty() {
        return Err(io::Error::other(format!(
            "Errors occurred:\n{}",
            errors.join("\n")
        )));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
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
    if options.remove_destination {
        let _ = tokio::fs::remove_file(destination).await;
    }
    if options.interactive
        && tokio::fs::try_exists(destination).await.unwrap_or(false)
        && !prompt_overwrite(destination)?
    {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    match fast_copy(source, destination, file_size, overall_pb, options) {
        Ok(true) => {
            let completed = completed_files.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(pb) = overall_pb
                && matches!(style, ProgressBarStyle::Default)
            {
                pb.set_message(format!("Copying: {}/{} files", completed, total_files));
            }
            if options.preserve != PreserveAttr::none() {
                preserve::apply_preserve_attrs(source, destination, options.preserve).await?;
            }
            return Ok(());
        }
        Ok(false) | Err(_) => {
            // Fallback to regular
        }
    }
    let mut src_file = tokio::fs::File::open(source).await?;
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

    let mut dest_file = tokio::io::BufWriter::with_capacity(buffer_size, dest_file);

    let mut buffer = vec![0u8; buffer_size];

    const MAX_UPDATES: u64 = 16;
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
    if accumulated_bytes > 0
        && let Some(pb) = overall_pb
    {
        pb.inc(accumulated_bytes);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{CopyOptions, SymlinkMode};
    use crate::utility::progress_bar::ProgressBarStyle;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    use tempfile::TempDir;

    fn create_test_file(path: &std::path::Path, content: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
    }

    #[tokio::test]
    async fn test_copy_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        let content = b"Hello, World!";
        create_test_file(&source, content).unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());
        let dest_content = fs::read(&dest).unwrap();
        assert_eq!(dest_content, content);
    }

    #[tokio::test]
    async fn test_copy_file_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");

        create_test_file(&source, b"content").unwrap();
        fs::create_dir(&dest_dir).unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest_dir, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
        assert_eq!(fs::read(&dest_file).unwrap(), b"content");
    }

    #[tokio::test]
    async fn test_copy_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("large.bin");
        let dest = temp_dir.path().join("large_copy.bin");

        let content = vec![0xAB; 10 * 1024 * 1024];
        create_test_file(&source, &content).unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());
        assert_eq!(fs::metadata(&dest).unwrap().len(), content.len() as u64);
    }

    #[tokio::test]
    async fn test_copy_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("empty.txt");
        let dest = temp_dir.path().join("empty_copy.txt");

        create_test_file(&source, b"").unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());
        assert_eq!(fs::metadata(&dest).unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_copy_directory_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        create_test_file(&source_dir.join("file1.txt"), b"content1").unwrap();
        create_test_file(&source_dir.join("file2.txt"), b"content2").unwrap();

        let subdir = source_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();
        create_test_file(&subdir.join("file3.txt"), b"content3").unwrap();

        let mut options = CopyOptions::none();
        options.recursive = true;

        copy(&source_dir, &dest_dir, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest_dir.join("source").is_dir());
        assert!(dest_dir.join("source/file1.txt").exists());
        assert!(dest_dir.join("source/file2.txt").exists());
        assert!(dest_dir.join("source/subdir/file3.txt").exists());

        assert_eq!(
            fs::read(dest_dir.join("source/file1.txt")).unwrap(),
            b"content1"
        );
    }

    #[tokio::test]
    async fn test_copy_directory_without_recursive_fails() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir(&source_dir).unwrap();

        let options = CopyOptions::none();
        let result = copy(&source_dir, &dest_dir, ProgressBarStyle::Minimal, &options).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("use -r to copy recursively")
        );
    }

    #[tokio::test]
    async fn test_copy_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("a/b/c/d");
        fs::create_dir_all(&source).unwrap();
        create_test_file(&source.join("deep.txt"), b"deep file").unwrap();

        let dest = temp_dir.path().join("dest");

        let mut options = CopyOptions::none();
        options.recursive = true;

        copy(
            &temp_dir.path().join("a"),
            &dest,
            ProgressBarStyle::Minimal,
            &options,
        )
        .await
        .unwrap();

        assert!(dest.join("a/b/c/d/deep.txt").exists());
    }

    #[tokio::test]
    async fn test_multiple_copy() {
        let temp_dir = TempDir::new().unwrap();
        let dest_dir = temp_dir.path().join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file3 = temp_dir.path().join("file3.txt");

        create_test_file(&file1, b"content1").unwrap();
        create_test_file(&file2, b"content2").unwrap();
        create_test_file(&file3, b"content3").unwrap();

        let sources = vec![file1, file2, file3];
        let options = CopyOptions::none();

        multiple_copy(
            sources,
            dest_dir.clone(),
            ProgressBarStyle::Minimal,
            &options,
        )
        .await
        .unwrap();

        assert!(dest_dir.join("file1.txt").exists());
        assert!(dest_dir.join("file2.txt").exists());
        assert!(dest_dir.join("file3.txt").exists());
    }

    #[tokio::test]
    async fn test_multiple_copy_with_directories() {
        let temp_dir = TempDir::new().unwrap();
        let dest_dir = temp_dir.path().join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let file1 = temp_dir.path().join("file.txt");
        create_test_file(&file1, b"file content").unwrap();

        let dir1 = temp_dir.path().join("dir1");
        fs::create_dir(&dir1).unwrap();
        create_test_file(&dir1.join("file_in_dir.txt"), b"dir content").unwrap();

        let sources = vec![file1, dir1];
        let mut options = CopyOptions::none();
        options.recursive = true;

        multiple_copy(
            sources,
            dest_dir.clone(),
            ProgressBarStyle::Minimal,
            &options,
        )
        .await
        .unwrap();

        assert!(dest_dir.join("file.txt").exists());
        assert!(dest_dir.join("dir1/file_in_dir.txt").exists());
    }

    #[tokio::test]
    async fn test_copy_with_force_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        create_test_file(&source, b"new content").unwrap();
        create_test_file(&dest, b"old content").unwrap();

        let mut options = CopyOptions::none();
        options.force = true;

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"new content");
    }

    #[tokio::test]
    async fn test_copy_without_force_fails_on_existing() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        create_test_file(&source, b"new content").unwrap();
        create_test_file(&dest, b"old content").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest).unwrap().permissions();
            perms.set_mode(0o444);
            fs::set_permissions(&dest, perms).unwrap();
        }

        let options = CopyOptions::none();
        let result = copy(&source, &dest, ProgressBarStyle::Minimal, &options).await;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&dest, perms).unwrap();
        }

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_copy_with_remove_destination() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        create_test_file(&source, b"new content").unwrap();
        create_test_file(&dest, b"old content").unwrap();

        let mut options = CopyOptions::none();
        options.remove_destination = true;

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"new content");
    }

    #[tokio::test]
    async fn test_copy_with_resume_skips_existing() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        create_test_file(&source_dir.join("file1.txt"), b"content1").unwrap();
        create_test_file(&source_dir.join("file2.txt"), b"content2").unwrap();

        create_test_file(&dest_dir.join("source/file1.txt"), b"content1").unwrap();

        let mut options = CopyOptions::none();
        options.recursive = true;
        options.resume = true;

        copy(&source_dir, &dest_dir, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest_dir.join("source/file1.txt").exists());
        assert!(dest_dir.join("source/file2.txt").exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_hardlink_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        create_test_file(&source, b"test content").unwrap();

        let mut options = CopyOptions::none();
        options.hard_link = true;

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());

        let source_meta = fs::metadata(&source).unwrap();
        let dest_meta = fs::metadata(&dest).unwrap();
        assert_eq!(source_meta.ino(), dest_meta.ino());
        assert_eq!(source_meta.nlink(), 2);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_hardlink_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let dest_dir = temp_dir.path().join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        create_test_file(&file1, b"content1").unwrap();
        create_test_file(&file2, b"content2").unwrap();

        let sources = vec![file1.clone(), file2.clone()];

        let mut options = CopyOptions::none();
        options.hard_link = true;

        multiple_copy(
            sources,
            dest_dir.clone(),
            ProgressBarStyle::Minimal,
            &options,
        )
        .await
        .unwrap();

        assert_eq!(
            fs::metadata(&file1).unwrap().ino(),
            fs::metadata(dest_dir.join("file1.txt")).unwrap().ino()
        );
        assert_eq!(
            fs::metadata(&file2).unwrap().ino(),
            fs::metadata(dest_dir.join("file2.txt")).unwrap().ino()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_hardlink_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        create_test_file(&source_dir.join("file.txt"), b"content").unwrap();

        let mut options = CopyOptions::none();
        options.hard_link = true;
        options.recursive = true;

        copy(&source_dir, &dest_dir, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        let source_file = source_dir.join("file.txt");
        let dest_file = dest_dir.join("source/file.txt");

        assert_eq!(
            fs::metadata(&source_file).unwrap().ino(),
            fs::metadata(&dest_file).unwrap().ino()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_hardlink_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        create_test_file(&source, b"new").unwrap();
        create_test_file(&dest, b"old").unwrap();

        let mut options = CopyOptions::none();
        options.hard_link = true;
        options.force = true;

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert_eq!(
            fs::metadata(&source).unwrap().ino(),
            fs::metadata(&dest).unwrap().ino()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_symlink_single_file_auto() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("link.txt");

        create_test_file(&source, b"content").unwrap();

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Auto);

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());
        assert!(dest.symlink_metadata().unwrap().is_symlink());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_symlink_absolute_mode() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("link.txt");

        create_test_file(&source, b"content").unwrap();

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Absolute);

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        let link_target = fs::read_link(&dest).unwrap();
        assert!(link_target.is_absolute());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_symlink_relative_mode() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("links");
        fs::create_dir(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        create_test_file(&source, b"content").unwrap();

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Relative);

        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        let link_target = fs::read_link(&dest).unwrap();
        assert!(!link_target.is_absolute());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_symlink_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).unwrap();
        create_test_file(&source_dir.join("file.txt"), b"content").unwrap();

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Auto);
        options.recursive = true;

        copy(&source_dir, &dest_dir, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(
            dest_dir
                .join("source/file.txt")
                .symlink_metadata()
                .unwrap()
                .is_symlink()
        );
    }

    #[tokio::test]
    async fn test_copy_nonexistent_source() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("dest.txt");

        let options = CopyOptions::none();
        let result = copy(&source, &dest, ProgressBarStyle::Minimal, &options).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_copy_to_file_when_expecting_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_file = temp_dir.path().join("dest.txt");

        fs::create_dir(&source_dir).unwrap();
        create_test_file(&dest_file, b"existing").unwrap();

        let mut options = CopyOptions::none();
        options.recursive = true;

        let result = copy(&source_dir, &dest_file, ProgressBarStyle::Minimal, &options).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file"));
    }

    #[tokio::test]
    async fn test_concurrent_copy() {
        let temp_dir = TempDir::new().unwrap();
        let dest_dir = temp_dir.path().join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let mut sources = Vec::new();
        for i in 0..10 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            create_test_file(&file, format!("content{}", i).as_bytes()).unwrap();
            sources.push(file);
        }

        let mut options = CopyOptions::none();
        options.concurrency = 4;

        multiple_copy(
            sources,
            dest_dir.clone(),
            ProgressBarStyle::Minimal,
            &options,
        )
        .await
        .unwrap();

        for i in 0..10 {
            assert!(dest_dir.join(format!("file{}.txt", i)).exists());
        }
    }

    #[tokio::test]
    async fn test_copy_with_different_progress_styles() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest1 = temp_dir.path().join("dest1.txt");
        let dest2 = temp_dir.path().join("dest2.txt");

        create_test_file(&source, b"content").unwrap();

        let options = CopyOptions::none();

        copy(&source, &dest1, ProgressBarStyle::Default, &options)
            .await
            .unwrap();
        copy(&source, &dest2, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest1.exists());
        assert!(dest2.exists());
    }

    #[tokio::test]
    async fn test_copy_file_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("file with spaces & special!.txt");
        let dest = temp_dir.path().join("dest with spaces.txt");

        create_test_file(&source, b"content").unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert!(dest.exists());
    }

    #[tokio::test]
    async fn test_copy_preserves_file_size() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.bin");
        let dest = temp_dir.path().join("dest.bin");

        let content = vec![0xFF; 12345];
        create_test_file(&source, &content).unwrap();

        let options = CopyOptions::none();
        copy(&source, &dest, ProgressBarStyle::Minimal, &options)
            .await
            .unwrap();

        assert_eq!(
            fs::metadata(&source).unwrap().len(),
            fs::metadata(&dest).unwrap().len()
        );
    }
}
