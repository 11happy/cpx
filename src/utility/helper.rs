use super::preprocess::{SymlinkKind, SymlinkTask};
use super::progress_bar::{ProgressBarStyle, ProgressOptions};
use crate::cli::args::{BackupMode, CopyOptions, FollowSymlink, ReflinkMode, SymlinkMode};
use crate::config::schema::Config;
use crate::error::{CopyError, CopyResult};
use crate::utility::preprocess::{FifoTask, HardlinkTask};
use std::io;
use std::path::{Path, PathBuf};

pub fn create_directories(
    dirs: &[crate::utility::preprocess::DirectoryTask],
    verbose: bool,
) -> io::Result<()> {
    let mut dirs: Vec<_> = dirs.iter().collect();
    dirs.sort_unstable_by_key(|d| d.destination.components().count());
    dirs.dedup_by_key(|d| &d.destination);

    for dir in &dirs {
        match std::fs::create_dir(&dir.destination) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir_all(&dir.destination)?;
            }
            Err(e) => return Err(e),
        }
        // -v reports directories as GNU cp does: only the ones it created.
        if verbose && let Some(src) = &dir.source {
            println!("'{}' -> '{}'", src.display(), dir.destination.display());
        }
    }
    Ok(())
}

/// True when both paths name the same directory entry (same parent
/// directory and same file name), as opposed to another link to the inode.
pub fn same_dir_entry(a: &Path, b: &Path) -> bool {
    fn key(p: &Path) -> Option<(PathBuf, std::ffi::OsString)> {
        let name = p.file_name()?.to_os_string();
        let parent = match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.canonicalize().ok()?,
            _ => Path::new(".").canonicalize().ok()?,
        };
        Some((parent, name))
    }
    matches!((key(a), key(b)), (Some(ka), Some(kb)) if ka == kb)
}

/// Create (or truncate) the destination like File::create, but with the
/// source's permission bits so the kernel applies `mode & !umask`.
#[cfg(unix)]
pub fn create_with_mode(path: &Path, mode: u32) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
}

#[cfg(not(unix))]
pub fn create_with_mode(path: &Path, _mode: u32) -> io::Result<std::fs::File> {
    std::fs::File::create(path)
}

/// The process umask. umask(2) can only be read by setting it, so this is
/// read once, before any copy thread exists, and cached.
#[cfg(unix)]
pub fn process_umask() -> u32 {
    use std::sync::OnceLock;
    static UMASK: OnceLock<u32> = OnceLock::new();
    *UMASK.get_or_init(|| {
        let old = unsafe { libc::umask(0) };
        unsafe { libc::umask(old) };
        old as u32
    })
}

#[cfg(not(unix))]
pub fn process_umask() -> u32 {
    0o022
}

/// True when both paths resolve to the same inode (following symlinks).
#[cfg(unix)]
pub fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// True when `path` is itself (not via a symlink) the same inode as `target`.
#[cfg(unix)]
fn is_inode_of(path: &Path, target: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(path), std::fs::metadata(target)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_inode_of(path: &Path, target: &Path) -> bool {
    same_file(path, target)
}

pub fn create_symlink(task: &SymlinkTask, options: &CopyOptions) -> io::Result<()> {
    // `cpx -s foo foo` would replace foo with a link to itself; an existing
    // symlink to foo at the destination is fine to replace.
    if task.kind != SymlinkKind::PreserveExact && is_inode_of(&task.destination, &task.source) {
        return Err(io::Error::other(format!(
            "'{}' and '{}' are the same file",
            task.source.display(),
            task.destination.display()
        )));
    }
    if task.destination.is_symlink() || task.destination.try_exists().unwrap_or(false) {
        if options.interactive && !prompt_overwrite(&task.destination).map_err(io::Error::other)? {
            return Ok(());
        }
        if options.attributes_only && !options.remove_destination {
            // --attributes-only must never remove destination data.
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("destination already exists: {:?}", task.destination),
            ));
        }
        // Like GNU cp, an existing non-directory destination is replaced.
        std::fs::remove_file(&task.destination)?;
    }
    let target = match task.kind {
        // `source` is the original symlink; reproduce its target verbatim.
        SymlinkKind::PreserveExact => std::fs::read_link(&task.source)?,
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

    if options.preserve.timestamps
        && task.kind == SymlinkKind::PreserveExact
        && let Ok(meta) = std::fs::symlink_metadata(&task.source)
    {
        let _ = filetime::set_symlink_file_times(
            &task.destination,
            filetime::FileTime::from_last_access_time(&meta),
            filetime::FileTime::from_last_modification_time(&meta),
        );
    }

    Ok(())
}

pub fn create_hardlink(task: &HardlinkTask, options: &CopyOptions) -> CopyResult<()> {
    if same_file(&task.source, &task.destination) {
        // `cp -l foo foo` is a no-op in GNU cp.
        return Ok(());
    }
    if task.destination.try_exists()? {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }

        if options.force || options.remove_destination || options.resume {
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

/// Recreate a named pipe at the destination (recursive copies only, like GNU cp -R).
#[cfg(unix)]
pub fn create_fifo(task: &FifoTask, options: &CopyOptions) -> CopyResult<()> {
    if task.destination.symlink_metadata().is_ok() {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }
        std::fs::remove_file(&task.destination)?;
    }
    let path = std::ffi::CString::new(task.destination.as_os_str().as_encoded_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if unsafe { libc::mkfifo(path.as_ptr(), task.mode as libc::mode_t) } != 0 {
        return Err(CopyError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn create_fifo(task: &FifoTask, _options: &CopyOptions) -> CopyResult<()> {
    Err(CopyError::CopyFailed {
        source: task.source.clone(),
        destination: task.destination.clone(),
        reason: "named pipes are not supported on this platform".to_string(),
    })
}

pub fn prompt_overwrite(path: &Path) -> io::Result<bool> {
    use std::io::{Write, stdin, stdout};

    print!("overwrite '{}'? (y/n): ", path.display());
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
        let options = CopyOptions::none();

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        create_symlink(&task, &options).unwrap();

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
        let options = CopyOptions::none();

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task, &options).unwrap();

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
        let options = CopyOptions::none();

        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("file.txt"), b"content").unwrap();

        let task = SymlinkTask {
            source: source_dir.clone(),
            destination: dest_link.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        create_symlink(&task, &options).unwrap();

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
        let options = CopyOptions::none();

        let dest_dir = temp_dir.path().join("x/y/z");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task, &options).unwrap();

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
        let options = CopyOptions::none();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::AbsoluteToSource,
        };

        let result = create_symlink(&task, &options);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_create_symlink_nonexistent_source_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("link.txt");
        let options = CopyOptions::none();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            kind: SymlinkKind::RelativeToSource,
        };

        create_symlink(&task, &options).unwrap();
        assert!(dest.symlink_metadata().unwrap().is_symlink());
        assert!(dest.metadata().is_err());
    }
}
