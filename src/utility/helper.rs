use crate::cli::args::CopyOptions;
use crate::utility::preprocess::HardlinkTask;

use super::preprocess::SymlinkTask;
use std::io;
use std::path::{Path, PathBuf};

pub fn create_directories(dirs: &[crate::utility::preprocess::DirectoryTask]) -> io::Result<()> {
    let mut dirs: Vec<_> = dirs.iter().collect();
    dirs.sort_by_key(|d| d.destination.components().count());
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

pub async fn create_symlink(task: &SymlinkTask) -> io::Result<()> {
    let target = if task.use_absolute {
        task.source.canonicalize()?
    } else {
        let dest_parent = task.destination.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Invalid destination path")
        })?;

        pathdiff::diff_paths(&task.source, dest_parent).ok_or_else(|| {
            io::Error::other(format!(
                "Cannot create relative path from {:?} to {:?}",
                dest_parent, task.source
            ))
        })?
    };

    #[cfg(unix)]
    {
        tokio::fs::symlink(&target, &task.destination).await?;
    }

    #[cfg(windows)]
    {
        // On Windows, we need to know if it's a file or directory
        let metadata = tokio::fs::metadata(&task.source).await?;
        if metadata.is_dir() {
            tokio::fs::symlink_dir(&target, &task.destination).await?;
        } else {
            tokio::fs::symlink_file(&target, &task.destination).await?;
        }
    }

    Ok(())
}

pub async fn create_hardlink(task: &HardlinkTask, options: &CopyOptions) -> io::Result<()> {
    if tokio::fs::try_exists(&task.destination).await? {
        if options.interactive && !prompt_overwrite(&task.destination)? {
            return Ok(());
        }

        if options.force || options.remove_destination {
            if let Err(e) = tokio::fs::remove_file(&task.destination).await {
                return Err(io::Error::new(
                    e.kind(),
                    format!(
                        "Cannot remove existing file '{}': {}",
                        task.destination.display(),
                        e
                    ),
                ));
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Destination '{}' already exists",
                    task.destination.display()
                ),
            ));
        }
    }
    tokio::fs::hard_link(&task.source, &task.destination)
        .await
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "Failed to create hard link '{}' -> '{}': {}",
                    task.source.display(),
                    task.destination.display(),
                    e
                ),
            )
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

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("link.txt");

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            use_absolute: true,
        };

        create_symlink(&task).await.unwrap();

        assert!(dest.exists());
        assert!(dest.symlink_metadata().unwrap().is_symlink());

        let link_target = fs::read_link(&dest).unwrap();
        assert!(link_target.is_absolute());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("links");
        fs::create_dir(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        fs::write(&source, b"test content").unwrap();

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            use_absolute: false,
        };

        create_symlink(&task).await.unwrap();

        assert!(dest.exists());
        assert!(dest.symlink_metadata().unwrap().is_symlink());

        let link_target = fs::read_link(&dest).unwrap();
        assert!(!link_target.is_absolute());
        assert_eq!(link_target, PathBuf::from("../source.txt"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_to_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source_dir");
        let dest_link = temp_dir.path().join("link_dir");

        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("file.txt"), b"content").unwrap();

        let task = SymlinkTask {
            source: source_dir.clone(),
            destination: dest_link.clone(),
            use_absolute: true,
        };

        create_symlink(&task).await.unwrap();

        assert!(dest_link.exists());
        assert!(dest_link.symlink_metadata().unwrap().is_symlink());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("a/b/c/source.txt");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"test").unwrap();

        let dest_dir = temp_dir.path().join("x/y/z");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("link.txt");

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            use_absolute: false,
        };

        create_symlink(&task).await.unwrap();

        assert!(dest.exists());
        let link_target = fs::read_link(&dest).unwrap();
        assert!(!link_target.is_absolute());
        assert_eq!(link_target, PathBuf::from("../../../a/b/c/source.txt"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_nonexistent_source_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("link.txt");

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            use_absolute: true,
        };

        let result = create_symlink(&task).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_create_symlink_nonexistent_source_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("nonexistent.txt");
        let dest = temp_dir.path().join("link.txt");

        let task = SymlinkTask {
            source: source.clone(),
            destination: dest.clone(),
            use_absolute: false,
        };

        create_symlink(&task).await.unwrap();
        assert!(dest.symlink_metadata().unwrap().is_symlink());
        assert!(dest.metadata().is_err());
    }
}
