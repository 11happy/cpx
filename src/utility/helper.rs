use std::path::{Path, PathBuf};
use tokio::io;

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
    use std::path::PathBuf;

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
}
