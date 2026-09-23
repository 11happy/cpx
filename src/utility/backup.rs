use crate::cli::args::BackupMode;
use crate::error::{CopyError, CopyResult};
use std::io;
use std::path::{Path, PathBuf};

const DEFAULT_SUFFIX: &str = "~";

pub fn generate_backup_path(destination: &Path, mode: BackupMode) -> CopyResult<PathBuf> {
    match mode {
        BackupMode::None => Ok(destination.to_path_buf()),
        BackupMode::Simple => Ok(add_suffix(destination)),
        BackupMode::Numbered => {
            let max_number =
                find_max_backup_number(destination).map_err(|e| CopyError::CopyFailed {
                    source: PathBuf::new(),
                    destination: destination.to_path_buf(),
                    reason: format!("Failed to find backup number: {}", e),
                })?;
            Ok(format_numbered_backup(destination, max_number + 1))
        }
        BackupMode::Existing => {
            let max_number =
                find_max_backup_number(destination).map_err(|e| CopyError::CopyFailed {
                    source: PathBuf::new(),
                    destination: destination.to_path_buf(),
                    reason: format!("Failed to find backup number: {}", e),
                })?;
            if max_number > 0 {
                Ok(format_numbered_backup(destination, max_number + 1))
            } else {
                Ok(add_suffix(destination))
            }
        }
    }
}

fn find_max_backup_number(path: &Path) -> io::Result<u32> {
    let parent = match path.parent() {
        // A bare file name has the empty path as parent; read_dir("") fails.
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?
        .to_string_lossy();
    let pattern_prefix = format!("{}.~", file_name);

    let mut max_number = 0u32;

    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        let entry_name = entry.file_name();
        let entry_name_str = entry_name.to_string_lossy();

        if entry_name_str.starts_with(&pattern_prefix) && entry_name_str.ends_with('~') {
            let num_part = &entry_name_str[pattern_prefix.len()..entry_name_str.len() - 1];
            if let Ok(num) = num_part.parse::<u32>() {
                max_number = max_number.max(num);
            }
        }
    }

    Ok(max_number)
}

fn add_suffix(path: &Path) -> PathBuf {
    let mut path_str = path.as_os_str().to_string_lossy().to_string();
    path_str.push_str(DEFAULT_SUFFIX);
    PathBuf::from(path_str)
}

fn format_numbered_backup(path: &Path, number: u32) -> PathBuf {
    let mut path_str = path.as_os_str().to_string_lossy().to_string();
    path_str.push_str(&format!(".~{}~", number));
    PathBuf::from(path_str)
}

pub fn create_backup(destination: &Path, backup_path: &PathBuf) -> CopyResult<()> {
    std::fs::rename(destination, backup_path).map_err(|e| CopyError::CopyFailed {
        source: destination.to_path_buf(),
        destination: backup_path.clone(),
        reason: format!("Failed to create backup: {}", e),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_add_suffix() {
        let path = Path::new("/tmp/file.txt");
        let result = add_suffix(path);
        assert_eq!(result, PathBuf::from("/tmp/file.txt~"));
    }

    #[test]
    fn test_format_numbered_backup() {
        let path = Path::new("/tmp/file.txt");
        let result = format_numbered_backup(path, 1);
        assert_eq!(result, PathBuf::from("/tmp/file.txt.~1~"));
    }

    #[test]
    fn test_find_max_backup_number_no_backups() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        fs::write(&file, "content").unwrap();

        let max = find_max_backup_number(&file).unwrap();
        assert_eq!(max, 0);
    }

    #[test]
    fn test_find_max_backup_number_with_backups() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        fs::write(&file, "content").unwrap();

        fs::write(temp_dir.path().join("test.txt.~1~"), "backup1").unwrap();
        fs::write(temp_dir.path().join("test.txt.~3~"), "backup3").unwrap();
        fs::write(temp_dir.path().join("test.txt.~2~"), "backup2").unwrap();

        let max = find_max_backup_number(&file).unwrap();
        assert_eq!(max, 3);
    }

    #[test]
    fn test_generate_backup_path_simple() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");

        let backup = generate_backup_path(&file, BackupMode::Simple).unwrap();
        assert_eq!(backup, add_suffix(&file));
    }

    #[test]
    fn test_generate_backup_path_numbered() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");

        let backup1 = generate_backup_path(&file, BackupMode::Numbered).unwrap();
        assert!(backup1.to_string_lossy().contains(".~1~"));

        fs::write(&backup1, "backup1").unwrap();

        let backup2 = generate_backup_path(&file, BackupMode::Numbered).unwrap();
        assert!(backup2.to_string_lossy().contains(".~2~"));
    }

    #[test]
    fn test_generate_backup_path_existing_no_numbered() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        fs::write(&file, "content").unwrap();

        let backup = generate_backup_path(&file, BackupMode::Existing).unwrap();
        assert_eq!(backup, add_suffix(&file));
    }

    #[test]
    fn test_generate_backup_path_existing_with_numbered() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("test.txt");
        fs::write(&file, "content").unwrap();

        let backup1 = temp_dir.path().join("test.txt.~1~");
        fs::write(&backup1, "backup1").unwrap();

        let backup = generate_backup_path(&file, BackupMode::Existing).unwrap();
        assert!(backup.to_string_lossy().contains(".~2~"));
    }
}
