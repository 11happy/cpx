use super::preprocess::{SymlinkKind, SymlinkTask};
use super::progress_bar::{ProgressBarStyle, ProgressOptions};
use crate::cli::args::{BackupMode, CopyOptions, FollowSymlink, ReflinkMode, SymlinkMode};
use crate::config::schema::Config;
use crate::error::{CopyError, CopyResult};
use crate::utility::backup::{create_backup, generate_backup_path};
use crate::utility::preprocess::HardlinkTask;
use indicatif::ProgressBar;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn create_directories(dirs: &[crate::utility::preprocess::DirectoryTask]) -> io::Result<()> {
    let mut dirs: Vec<_> = dirs.iter().collect();
    dirs.sort_unstable_by_key(|d| d.destination.components().count());
    dirs.dedup_by_key(|d| &d.destination);

    for dir in &dirs {
        match std::fs::create_dir(&dir.destination) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir_all(&dir.destination)?;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn create_symlink(task: &SymlinkTask) -> io::Result<()> {
    let target = match task.kind {
        SymlinkKind::PreserveExact => task.source.clone(),
        SymlinkKind::AbsoluteToSource => task.source.canonicalize()?,
        SymlinkKind::RelativeToSource => {
            let dest_parent = task.destination.parent().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid destination path")
            })?;
            pathdiff::diff_paths(&task.source, dest_parent).ok_or_else(|| {
                io::Error::other(format!(
                    "Cannot create relative path from {:?} to {:?}",
                    dest_parent, task.source
                ))
            })?
        }
    };

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, &task.destination)?;
    }

    #[cfg(windows)]
    {
        let meta = std::fs::metadata(&target).ok();
        if meta.as_ref().map_or(false, |m| m.is_dir()) {
            std::os::windows::fs::symlink_dir(&target, &task.destination)?;
        } else {
            std::os::windows::fs::symlink_file(&target, &task.destination)?;
        }
    }

    Ok(())
}

pub fn create_hardlink(task: &HardlinkTask, options: &CopyOptions) -> CopyResult<()> {
    if task.destination.try_exists()? {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }

        if options.force || options.remove_destination {
            if let Err(_e) = std::fs::remove_file(&task.destination) {
                return Err(CopyError::HardlinkFailed {
                    source: task.source.clone(),
                    destination: task.destination.clone(),
                });
            }
        } else {
            return Err(CopyError::FileExists(task.destination.clone()));
        }
    }

    std::fs::hard_link(&task.source, &task.destination).map_err(|_e| {
        CopyError::HardlinkFailed {
            source: task.source.clone(),
            destination: task.destination.clone(),
        }
    })?;

    Ok(())
}

pub fn prompt_overwrite(path: &Path) -> io::Result<bool> {
    use std::io::{Write, stdin, stdout};

    print!("overwrite '{}'? (y/n): ", path.display());
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

pub fn prompt_deletion(path: &Path) -> io::Result<bool> {
    use std::io::{Write, stdin, stdout};

    if path.is_dir() {
        print!("delete all the files in '{}' (y/n): ", path.display());
    } else {
        print!("delete file '{}' (y/n): ", path.display());
    }

    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

pub fn with_parents(dest: &Path, source: &Path) -> PathBuf {
    let skip_count = if source.is_absolute() { 1 } else { 0 };
    let components = source.components().skip(skip_count);

    let mut relative = PathBuf::new();
    for comp in components {
        relative.push(comp.as_os_str());
    }

    dest.join(relative)
}

pub fn truncate_filename(filename: &str, max_len: usize) -> String {
    if filename.len() <= max_len {
        filename.to_string()
    } else {
        let truncate_at = max_len.saturating_sub(3);
        format!("{}...", &filename[..truncate_at])
    }
}

pub fn parse_symlink_mode(s: &str) -> Option<SymlinkMode> {
    match s {
        "auto" => Some(SymlinkMode::Auto),
        "absolute" => Some(SymlinkMode::Absolute),
        "relative" => Some(SymlinkMode::Relative),
        _ => None,
    }
}

pub fn parse_follow_symlink(s: &str) -> FollowSymlink {
    match s {
        "never" => FollowSymlink::NoDereference,
        "always" => FollowSymlink::Dereference,
        "command-line" => FollowSymlink::CommandLineSymlink,
        _ => FollowSymlink::NoDereference,
    }
}

pub fn parse_progress_style(s: &str) -> ProgressBarStyle {
    match s {
        "detailed" => ProgressBarStyle::Detailed,
        _ => ProgressBarStyle::Default,
    }
}

pub fn parse_progress_bar(cfg: &Config) -> ProgressOptions {
    ProgressOptions {
        style: parse_progress_style(&cfg.progress.style),
        filled: cfg.progress.bar.filled.clone(),
        empty: cfg.progress.bar.empty.clone(),
        head: cfg.progress.bar.head.clone(),
        bar_color: cfg.progress.color.bar.clone(),
        message_color: cfg.progress.color.message.clone(),
    }
}

pub fn parse_backup_mode(s: &str) -> Option<BackupMode> {
    match s {
        "none" => Some(BackupMode::None),
        "simple" => Some(BackupMode::Simple),
        "numbered" => Some(BackupMode::Numbered),
        "existing" => Some(BackupMode::Existing),
        _ => None,
    }
}

pub fn parse_reflink_mode(s: &str) -> Option<ReflinkMode> {
    match s {
        "auto" => Some(ReflinkMode::Auto),
        "always" => Some(ReflinkMode::Always),
        "never" => Some(ReflinkMode::Never),
        _ => None,
    }
}

pub fn move_file(
    source: &Path,
    destination: &Path,
    overall_pb: Option<&ProgressBar>,
    completed_files: &AtomicUsize,
    total_files: usize,
    options: &CopyOptions,
) -> CopyResult<()> {
    // Handle backup if needed
    if let Some(backup_mode) = options.backup
        && backup_mode != BackupMode::None
        && destination.try_exists().unwrap_or(false)
    {
        let backup_path = generate_backup_path(destination, backup_mode)?;
        let _ = create_backup(destination, &backup_path);
    }

    if options.remove_destination && destination.try_exists().unwrap_or(false) {
        let _ = std::fs::remove_file(destination);
    }

    // Get file size before rename for progress tracking
    let file_size = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);

    // Perform the rename
    std::fs::rename(source, destination).map_err(|e| CopyError::CopyFailed {
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        reason: format!("Failed to move file: {}", e),
    })?;

    // Update progress
    if let Some(pb) = overall_pb {
        pb.inc(file_size);
    }
    update_progress(overall_pb, completed_files, total_files, options);

    Ok(())
}

pub fn move_symlink(task: &SymlinkTask, options: &CopyOptions) -> CopyResult<()> {
    if task.destination.try_exists()? {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }

        if options.force || options.remove_destination {
            if let Err(_e) = std::fs::remove_file(&task.destination) {
                return Err(CopyError::SymlinkFailed {
                    source: task.symlink_path.clone(),
                    destination: task.destination.clone(),
                });
            }
        } else {
            return Err(CopyError::FileExists(task.destination.clone()));
        }
    }

    // Move the symlink file itself, not the target
    std::fs::rename(&task.symlink_path, &task.destination).map_err(|_e| {
        CopyError::SymlinkFailed {
            source: task.symlink_path.clone(),
            destination: task.destination.clone(),
        }
    })?;

    Ok(())
}

pub fn move_hardlink(task: &HardlinkTask, options: &CopyOptions) -> CopyResult<()> {
    if task.destination.try_exists()? {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }

        if options.force || options.remove_destination {
            if let Err(_e) = std::fs::remove_file(&task.destination) {
                return Err(CopyError::HardlinkFailed {
                    source: task.source.clone(),
                    destination: task.destination.clone(),
                });
            }
        } else {
            return Err(CopyError::FileExists(task.destination.clone()));
        }
    }

    // Create the hardlink at destination
    std::fs::hard_link(&task.source, &task.destination).map_err(|_e| {
        CopyError::HardlinkFailed {
            source: task.source.clone(),
            destination: task.destination.clone(),
        }
    })?;

    // Remove the source
    if let Err(_e) = std::fs::remove_file(&task.source) {
        eprintln!(
            "Warning: Could not remove source file {} after moving hardlink",
            task.source.display()
        );
    }

    Ok(())
}

pub fn update_progress(
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
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_with_parents_relative_path() {
        let dest = Path::new("/dest");
        let source = Path::new("a/b/file.txt");

        let result = with_parents(dest, source);
        assert_eq!(result, PathBuf::from("/dest/a/b/file.txt"));
    }

    #[test]
    fn test_with_parents_absolute_path_unix() {
        #[cfg(unix)]
        {
            let dest = Path::new("/dest");
            let source = Path::new("/home/user/file.txt");

            let result = with_parents(dest, source);
            assert_eq!(result, PathBuf::from("/dest/home/user/file.txt"));
        }
    }

    #[test]
    fn test_with_parents_single_file() {
        let dest = Path::new("/dest");
        let source = Path::new("file.txt");

        let result = with_parents(dest, source);
        assert_eq!(result, PathBuf::from("/dest/file.txt"));
    }

    #[test]
    fn test_with_parents_nested_path() {
        let dest = Path::new("/backup");
        let source = Path::new("projects/rust/cpx/src/main.rs");

        let result = with_parents(dest, source);
        assert_eq!(
            result,
            PathBuf::from("/backup/projects/rust/cpx/src/main.rs")
        );
    }

    #[test]
    fn test_with_parents_dest_with_trailing_slash() {
        let dest = Path::new("/dest/");
        let source = Path::new("a/b/file.txt");

        let result = with_parents(dest, source);
        assert_eq!(result, PathBuf::from("/dest/a/b/file.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn test_with_parents_root_in_source() {
        let dest = Path::new("/backup");
        let source = Path::new("/etc/config/app.conf");

        let result = with_parents(dest, source);
        assert_eq!(result, PathBuf::from("/backup/etc/config/app.conf"));
    }

    #[test]
    fn test_with_parents_current_dir() {
        let dest = Path::new("/dest");
        let source = Path::new("./file.txt");

        let result = with_parents(dest, source);
        assert!(result.to_string_lossy().ends_with("file.txt"));
    }

    #[test]
    fn test_with_parents_empty_dest() {
        let dest = Path::new("");
        let source = Path::new("a/b/file.txt");

        let result = with_parents(dest, source);
        assert_eq!(result, PathBuf::from("a/b/file.txt"));
    }

    #[test]
    fn test_truncate_filename_short() {
        let filename = "short.txt";
        let result = truncate_filename(filename, 20);
        assert_eq!(result, "short.txt");
    }

    #[test]
    fn test_truncate_filename_exact() {
        let filename = "exactly_ten";
        let result = truncate_filename(filename, 11);
        assert_eq!(result, "exactly_ten");
    }

    #[test]
    fn test_truncate_filename_long() {
        let filename = "this_is_a_very_long_filename.txt";
        let result = truncate_filename(filename, 15);
        assert_eq!(result, "this_is_a_ve...");
    }

    #[test]
    fn test_truncate_filename_zero_max() {
        let filename = "test.txt";
        let result = truncate_filename(filename, 0);
        assert_eq!(result, "...");
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("link.txt");

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            symlink_path: source.clone(),
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        create_symlink(&task).unwrap();

        assert!(dest.exists());
        assert!(dest.symlink_metadata().unwrap().is_symlink());

        let link_target = fs::read_link(&dest).unwrap();
        assert!(link_target.is_absolute());
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("links");
        fs::create_dir(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            symlink_path: source.clone(),
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task).unwrap();

        assert!(dest.exists());
        assert!(dest.symlink_metadata().unwrap().is_symlink());

        let link_target = fs::read_link(&dest).unwrap();
        assert!(!link_target.is_absolute());
        assert_eq!(link_target, PathBuf::from("../source.txt"));
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_link = temp_dir.path().join("link_dir");

        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("file.txt"), b"content").unwrap();

        let task = SymlinkTask {
            symlink_path: source_dir.clone(),
            source: source_dir.clone(),
            destination: dest_link.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        create_symlink(&task).unwrap();

        assert!(dest_link.exists());
        assert!(dest_link.symlink_metadata().unwrap().is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("a/b/c/source.txt");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"test").unwrap();

        let dest_dir = temp_dir.path().join("x/y/z");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        let task = SymlinkTask {
            symlink_path: source.clone(),
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task).unwrap();

        assert!(dest.exists());
        let link_target = fs::read_link(&dest).unwrap();
        assert!(!link_target.is_absolute());
        assert_eq!(link_target, PathBuf::from("../../../a/b/c/source.txt"));
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_nonexistent_source_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("link.txt");

        let task = SymlinkTask {
            symlink_path: source.clone(),
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        let result = create_symlink(&task);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_nonexistent_source_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("link.txt");

        let task = SymlinkTask {
            symlink_path: source.clone(),
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task).unwrap();
        assert!(dest.symlink_metadata().unwrap().is_symlink());
        assert!(dest.metadata().is_err());
    }
}
