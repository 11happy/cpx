use crate::cli::args::{BackupMode, CopyOptions, FollowSymlink};
#[cfg(target_os = "linux")]
use crate::core::fast_copy::{FastCopy, fast_copy};

use crate::error::{CopyError, CopyResult};
use crate::utility::backup::{create_backup, generate_backup_path};
use crate::utility::helper::{
    create_directories, create_fifo, create_hardlink, create_symlink, process_umask,
    prompt_overwrite, same_dir_entry, same_file, truncate_filename,
};
use crate::utility::preprocess::{
    CopyPlan, preprocess_directory, preprocess_file, preprocess_multiple,
};
use crate::utility::preserve::{self, PreserveAttr};
use crate::utility::progress_bar::ProgressBarStyle;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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

    let plan = if source_metadata.is_dir() {
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

    execute_copy(plan, options)
}

pub fn multiple_copy(
    sources: Vec<PathBuf>,
    destination: PathBuf,
    options: &CopyOptions,
) -> CopyResult<()> {
    let plan = preprocess_multiple(&sources, &destination, options).map_err(|e| {
        CopyError::CopyFailed {
            source: sources[0].clone(),
            destination: destination.clone(),
            reason: e.to_string(),
        }
    })?;
    if plan.skipped_files > 0 {
        eprintln!("Skipping {} files that already exist", plan.skipped_files);
    }
    execute_copy(plan, options)
}

/// Files at least this large get their own progress bar under the overall one.
const PER_FILE_BAR_MIN: u64 = 64 * 1024 * 1024;

/// `-v`: print one `'src' -> 'dst'` line, routed through the progress
/// display when one is active so the bar is not garbled.
fn log_verbose(multi: Option<&MultiProgress>, source: &Path, destination: &Path) {
    let line = format!("'{}' -> '{}'", source.display(), destination.display());
    match multi {
        Some(m) => m.suspend(|| println!("{}", line)),
        None => println!("{}", line),
    }
}

fn execute_copy(mut plan: CopyPlan, options: &CopyOptions) -> CopyResult<()> {
    // Read the umask now, while single-threaded (see process_umask).
    process_umask();

    if options.preserve.links {
        plan.link_duplicate_inodes();
    }

    if !options.attributes_only {
        create_directories(&plan.directories, options.verbose)?;
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

    if !plan.symlinks.is_empty() {
        for symlink_task in &plan.symlinks {
            create_symlink(symlink_task, options).map_err(|_e| CopyError::SymlinkFailed {
                source: symlink_task.source.clone(),
                destination: symlink_task.destination.clone(),
            })?;
            if options.verbose {
                log_verbose(None, &symlink_task.source, &symlink_task.destination);
            }
        }
        if plan.total_symlinks > 0 {
            println!("Created {} symbolic links", plan.total_symlinks);
        }

        if options.symbolic_link.is_some() {
            return Ok(());
        }
    }

    if !options.attributes_only {
        for fifo_task in &plan.fifos {
            create_fifo(fifo_task, options)?;
            if options.verbose {
                log_verbose(None, &fifo_task.source, &fifo_task.destination);
            }
        }
    }

    if options.hard_link {
        for hardlink_task in &plan.hardlinks {
            create_hardlink(hardlink_task, options)?;
            if options.verbose {
                log_verbose(None, &hardlink_task.source, &hardlink_task.destination);
            }
        }

        if plan.total_hardlinks > 0 {
            println!("Created {} hard links", plan.total_hardlinks);
        }
        return Ok(());
    }

    let (multi, overall_pb) =
        if plan.total_files >= 1 && !options.interactive && !options.attributes_only {
            let multi = MultiProgress::new();
            let pb = multi.add(ProgressBar::new(plan.total_size));
            options.progress_bar.apply(&pb, plan.total_files);
            (Some(multi), Some(Arc::new(pb)))
        } else {
            (None, None)
        };

    let completed_files = Arc::new(AtomicUsize::new(0));

    let mut declined = None;

    // For interactive mode, process sequentially
    if options.interactive {
        for file_task in &plan.files {
            match copy_core(
                &file_task.source,
                &file_task.destination,
                file_task.size,
                file_task.mode,
                file_task.sparse,
                multi.as_ref(),
                overall_pb.as_deref(),
                &completed_files,
                plan.total_files,
                options,
            ) {
                Ok(()) => {}
                Err(CopyError::OverwriteDeclined(path)) => declined = Some(path),
                Err(e) => return Err(e),
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

        // Files are sorted largest-first; every worker pulls the next one from a
        // shared counter (longest-processing-time-first scheduling). A plain
        // par_iter would hand one worker a contiguous chunk of the largest files
        // to copy sequentially while the others sit idle on the tail of small ones.
        let next = AtomicUsize::new(0);
        let per_thread_errors: Vec<Vec<(PathBuf, PathBuf, CopyError)>> = pool.broadcast(|_| {
            let mut errors = Vec::new();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(file_task) = plan.files.get(i) else {
                    break;
                };
                if let Err(e) = copy_core(
                    &file_task.source,
                    &file_task.destination,
                    file_task.size,
                    file_task.mode,
                    file_task.sparse,
                    multi.as_ref(),
                    overall_pb.as_deref(),
                    &completed_files,
                    plan.total_files,
                    options,
                ) {
                    errors.push((file_task.source.clone(), file_task.destination.clone(), e));
                }
            }
            errors
        });

        let mut interrupted = false;
        let mut errors: Vec<(PathBuf, PathBuf, CopyError)> = Vec::new();

        for (source, dest, e) in per_thread_errors.into_iter().flatten() {
            match e {
                CopyError::Io(ref io_err) if io_err.kind() == io::ErrorKind::Interrupted => {
                    interrupted = true;
                }
                _ => {
                    errors.push((source, dest, e));
                }
            }
        }

        create_preserved_links(&plan, options)?;
        preserve_directory_attrs(&plan, options)?;

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

    if options.interactive {
        create_preserved_links(&plan, options)?;
        preserve_directory_attrs(&plan, options)?;
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

    match declined {
        Some(path) => Err(CopyError::OverwriteDeclined(path)),
        None => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_core(
    source: &Path,
    destination: &Path,
    file_size: u64,
    file_mode: u32,
    maybe_sparse: bool,
    multi: Option<&MultiProgress>,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
) -> CopyResult<()> {
    if options.attributes_only {
        if std::fs::symlink_metadata(destination).is_err() {
            // GNU cp creates an empty destination to carry the attributes.
            std::fs::File::create(destination)?;
        }
        preserve::apply_preserve_attrs(source, destination, options.preserve)?;
        return Ok(());
    }

    // Refuse to copy a file onto itself (also via a symlink or hard link to
    // it): the destination would be truncated before it is read. GNU cp
    // allows it only with --force --backup, where the backup becomes the source.
    // One lstat of the destination answers the common case (absent) and
    // feeds the same-file and dangling-symlink checks below.
    let dest_lmeta = std::fs::symlink_metadata(destination).ok();
    let mut is_same_file = dest_lmeta.is_some() && same_file(source, destination);
    if is_same_file && options.remove_destination && !same_dir_entry(source, destination) {
        // A symlink or hard link to the source can be removed without
        // touching the source; the source's own directory entry cannot.
        std::fs::remove_file(destination)?;
        is_same_file = false;
    }
    let backup_of_self;
    let copy_from_backup =
        is_same_file && options.force && options.backup.is_some_and(|b| b != BackupMode::None);
    let source = if copy_from_backup {
        backup_of_self = generate_backup_path(destination, options.backup.unwrap())?;
        create_backup(destination, &backup_of_self)?;
        backup_of_self.as_path()
    } else {
        source
    };
    if is_same_file && !copy_from_backup {
        return Err(CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!(
                "'{}' and '{}' are the same file",
                source.display(),
                destination.display()
            ),
        });
    }

    if options.interactive
        && destination.try_exists().unwrap_or(false)
        && !prompt_overwrite(destination)?
    {
        return Err(CopyError::OverwriteDeclined(destination.to_path_buf()));
    }

    if let Some(backup_mode) = options.backup
        && backup_mode != BackupMode::None
        && destination.try_exists().unwrap_or(false)
        && !is_same_file
    {
        let backup_path = generate_backup_path(destination, backup_mode)?;
        if same_file(source, &backup_path) {
            return Err(CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!(
                    "backing up '{}' might destroy source; '{}' not copied",
                    destination.display(),
                    source.display()
                ),
            });
        }
        let _ = create_backup(destination, &backup_path);
    }

    if options.remove_destination {
        let _ = std::fs::remove_file(destination);
    } else if dest_lmeta.as_ref().is_some_and(|m| m.is_symlink())
        && let Err(e) = std::fs::metadata(destination)
    {
        if options.force && e.raw_os_error() == Some(libc::ELOOP) {
            // -f: a destination that cannot be opened is removed and retried.
            std::fs::remove_file(destination)?;
        } else {
            // GNU cp refuses to create the target of a dangling destination symlink.
            return Err(CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!(
                    "not writing through dangling symlink '{}'",
                    destination.display()
                ),
            });
        }
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
                    update_progress(
                        source,
                        destination,
                        multi,
                        overall_pb,
                        completed_files,
                        total_files,
                        options,
                    );
                    if options.preserve != PreserveAttr::none() {
                        preserve::apply_preserve_attrs(source, destination, options.preserve)
                            .map_err(CopyError::from)?;
                    }
                    return Ok(());
                }
                Err(_e) if reflink_mode == ReflinkMode::Always => {
                    return Err(CopyError::ReflinkFailed {
                        source: source.to_path_buf(),
                        destination: destination.to_path_buf(),
                    });
                }
                Err(_) => {}
            }
        }
    }

    let file_pb = match multi {
        Some(m) if file_size >= PER_FILE_BAR_MIN => Some(m.add(new_file_bar(source, file_size))),
        _ => None,
    };

    // A read-only source (e.g. 0444) is created writable first so that
    // xattrs and ACLs can still be applied, then narrowed in finish_file.
    let create_mode = file_mode | 0o200;

    #[cfg(target_os = "linux")]
    let (mut src_file, dest_file) = {
        if options.abort.load(Ordering::Relaxed) {
            return Err(CopyError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted by user",
            )));
        }
        match fast_copy(
            source,
            destination,
            file_size,
            create_mode,
            maybe_sparse,
            overall_pb,
            file_pb.as_ref(),
            options,
        )? {
            FastCopy::Done => {
                finish_file_bar(multi, file_pb);
                update_progress(
                    source,
                    destination,
                    multi,
                    overall_pb,
                    completed_files,
                    total_files,
                    options,
                );
                return finish_file(source, destination, file_mode, options);
            }
            FastCopy::Fallback(src_file, dest_file) => (src_file, dest_file),
        }
    };

    #[cfg(not(target_os = "linux"))]
    let (mut src_file, dest_file) = {
        use crate::utility::helper::create_with_mode;
        let src_file = std::fs::File::open(source)?;
        let dest_file = match create_with_mode(destination, create_mode) {
            Ok(file) => file,
            Err(_e) if options.force => {
                let _ = std::fs::remove_file(destination);
                create_with_mode(destination, create_mode)?
            }
            Err(e) => return Err(CopyError::Io(e)),
        };
        (src_file, dest_file)
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
            if let Some(pb) = &file_pb {
                pb.inc(accumulated_bytes);
            }
            accumulated_bytes = 0;
        }
    }

    if accumulated_bytes > 0 {
        if let Some(pb) = overall_pb {
            pb.inc(accumulated_bytes);
        }
        if let Some(pb) = &file_pb {
            pb.inc(accumulated_bytes);
        }
    }

    dest_file.flush()?;
    finish_file_bar(multi, file_pb);

    update_progress(
        source,
        destination,
        multi,
        overall_pb,
        completed_files,
        total_files,
        options,
    );

    finish_file(source, destination, file_mode, options)
}

/// Apply preserved attributes, then drop the owner-write bit added at
/// creation when the source did not have it (mode & !umask, like GNU cp).
fn finish_file(
    source: &Path,
    destination: &Path,
    file_mode: u32,
    options: &CopyOptions,
) -> CopyResult<()> {
    if options.preserve != PreserveAttr::none() {
        preserve::apply_preserve_attrs(source, destination, options.preserve)
            .map_err(CopyError::from)?;
    }
    #[cfg(unix)]
    if file_mode & 0o200 == 0 && !options.preserve.mode {
        use std::os::unix::fs::PermissionsExt;
        let mode = file_mode & !process_umask();
        std::fs::set_permissions(destination, std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

/// --preserve=links: re-link files that were hard links of an already
/// copied file. Runs after the copy phase so the link target exists.
fn create_preserved_links(plan: &CopyPlan, options: &CopyOptions) -> CopyResult<()> {
    for task in &plan.preserved_links {
        if task.destination.symlink_metadata().is_ok() {
            std::fs::remove_file(&task.destination)?;
        }
        std::fs::hard_link(&task.source, &task.destination).map_err(|_e| {
            CopyError::HardlinkFailed {
                source: task.source.clone(),
                destination: task.destination.clone(),
            }
        })?;
        if options.verbose {
            log_verbose(None, &task.source, &task.destination);
        }
    }
    Ok(())
}

/// Directory attributes go last and deepest-first: a read-only or
/// timestamp-preserved directory must not be touched before its contents.
fn preserve_directory_attrs(plan: &CopyPlan, options: &CopyOptions) -> CopyResult<()> {
    if options.preserve == PreserveAttr::none() || options.attributes_only {
        return Ok(());
    }
    for dir_task in plan.directories.iter().rev() {
        if let Some(src) = &dir_task.source
            && dir_task.destination.is_dir()
        {
            preserve::apply_preserve_attrs(src, &dir_task.destination, options.preserve).map_err(
                |e| CopyError::CopyFailed {
                    source: src.clone(),
                    destination: dir_task.destination.clone(),
                    reason: e.to_string(),
                },
            )?;
        }
    }
    Ok(())
}

fn new_file_bar(source: &Path, file_size: u64) -> ProgressBar {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "  {msg:<32} {bar:20} {binary_bytes}/{binary_total_bytes} {binary_bytes_per_sec}",
            )
            .unwrap(),
    );
    pb.set_message(truncate_filename(&name, 32));
    pb
}

fn finish_file_bar(multi: Option<&MultiProgress>, file_pb: Option<ProgressBar>) {
    if let (Some(m), Some(pb)) = (multi, file_pb) {
        pb.finish_and_clear();
        m.remove(&pb);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_progress(
    source: &Path,
    destination: &Path,
    multi: Option<&MultiProgress>,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
) {
    if options.verbose {
        log_verbose(multi, source, destination);
    }
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
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;
    fn default_copy_options() -> CopyOptions {
        CopyOptions {
            recursive: false,
            resume: false,
            verbose: false,
            update: false,
            no_clobber: false,
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
}
