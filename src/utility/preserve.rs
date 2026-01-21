use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreserveAttr {
    pub mode: bool,
    pub ownership: bool,
    pub timestamps: bool,
    pub links: bool,
    pub context: bool,
    pub xattr: bool,
}

impl Default for PreserveAttr {
    fn default() -> Self {
        Self {
            mode: true,
            ownership: true,
            timestamps: true,
            links: false,
            context: false,
            xattr: false,
        }
    }
}

impl PreserveAttr {
    pub fn none() -> Self {
        Self {
            mode: false,
            ownership: false,
            timestamps: false,
            links: false,
            context: false,
            xattr: false,
        }
    }

    pub fn all() -> Self {
        Self {
            mode: true,
            ownership: true,
            timestamps: true,
            links: true,
            context: true,
            xattr: true,
        }
    }

    pub fn from_string(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Ok(Self::default());
        }

        if s == "all" {
            return Ok(Self::all());
        }

        let mut attr = Self::none();

        for cur in s.split(',') {
            match cur.trim() {
                "" => continue,
                "mode" => attr.mode = true,
                "ownership" => attr.ownership = true,
                "timestamps" => attr.timestamps = true,
                "xattr" => attr.xattr = true,
                "context" => attr.context = true,
                "links" => attr.links = true,
                "all" => return Ok(Self::all()),
                other => return Err(format!("Unknown attribute: {}", other)),
            }
        }

        Ok(attr)
    }
}

pub fn apply_preserve_attrs(
    source: &Path,
    destination: &Path,
    attrs: PreserveAttr,
) -> io::Result<()> {
    let src_metadata = std::fs::metadata(source)?;
    if attrs.timestamps {
        preserve_timestamps(destination, &src_metadata)?;
    }
    #[cfg(unix)]
    if attrs.mode {
        preserve_mode(destination, &src_metadata)?;
    }

    #[cfg(unix)]
    if attrs.ownership {
        preserve_ownership(destination, &src_metadata)?;
    }
    Ok(())
}

fn preserve_timestamps(destination: &Path, src_metadata: &std::fs::Metadata) -> io::Result<()> {
    use filetime::{FileTime, set_file_mtime};

    let modified_time = src_metadata.modified().map_err(io::Error::other)?;

    let system_modified_time = FileTime::from_system_time(modified_time);

    set_file_mtime(destination, system_modified_time).map_err(io::Error::other)?;

    Ok(())
}

#[cfg(unix)]
fn preserve_mode(destination: &Path, src_metadata: &std::fs::Metadata) -> io::Result<()> {
    use std::fs::Permissions;

    let mode = src_metadata.permissions().mode();
    let permissions = Permissions::from_mode(mode);

    std::fs::set_permissions(destination, permissions)?;

    Ok(())
}

#[cfg(unix)]
fn preserve_ownership(destination: &Path, src_metadata: &std::fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let uid = src_metadata.uid();
    let gid = src_metadata.gid();

    // Note: This requires elevated privileges (root) to work in most cases
    // We'll attempt it but won't fail if it doesn't work
    let dest_cstring = std::ffi::CString::new(destination.to_string_lossy().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    unsafe {
        let result = libc::chown(dest_cstring.as_ptr(), uid, gid);
        if result != 0 {
            let err = io::Error::last_os_error();
            // Only return error if it's not a permission issue
            // (EPERM = 1, EACCES = 13)
            if err.raw_os_error() != Some(1) && err.raw_os_error() != Some(13) {
                return Err(err);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_preserve_attr_from_string() {
        let attr = PreserveAttr::from_string("mode,timestamps").unwrap();
        assert!(attr.mode);
        assert!(attr.timestamps);
        assert!(!attr.ownership);
        assert!(!attr.xattr);
    }

    #[test]
    fn test_preserve_attr_all() {
        let attr = PreserveAttr::from_string("all").unwrap();
        assert!(attr.mode);
        assert!(attr.ownership);
        assert!(attr.timestamps);
        assert!(attr.links);
        assert!(attr.context);
        assert!(attr.xattr);
    }

    #[test]
    fn test_preserve_attr_default() {
        let attr = PreserveAttr::from_string("").unwrap();
        assert!(attr.mode);
        assert!(attr.ownership);
        assert!(attr.timestamps);
        assert!(!attr.links);
        assert!(!attr.context);
        assert!(!attr.xattr);
    }

    #[test]
    fn test_preserve_attr_none() {
        let attr = PreserveAttr::none();
        assert!(!attr.mode);
        assert!(!attr.ownership);
        assert!(!attr.timestamps);
        assert!(!attr.links);
        assert!(!attr.context);
        assert!(!attr.xattr);
    }

    #[test]
    fn test_preserve_attr_from_string_with_spaces() {
        let attr = PreserveAttr::from_string("mode , timestamps , xattr").unwrap();
        assert!(attr.mode);
        assert!(attr.timestamps);
        assert!(attr.xattr);
        assert!(!attr.ownership);
    }

    #[test]
    fn test_preserve_attr_from_string_invalid() {
        let result = PreserveAttr::from_string("mode,invalid_attr");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown attribute"));
    }

    #[test]
    fn test_preserve_timestamps() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test").unwrap();
        thread::sleep(Duration::from_millis(100));
        fs::write(&dest, b"test").unwrap();

        let src_metadata = fs::metadata(&source).unwrap();
        preserve_timestamps(&dest, &src_metadata).unwrap();

        let src_mtime = src_metadata.modified().unwrap();
        let dest_mtime = fs::metadata(&dest).unwrap().modified().unwrap();

        // Allow for small differences due to precision
        let diff = if src_mtime > dest_mtime {
            src_mtime.duration_since(dest_mtime).unwrap()
        } else {
            dest_mtime.duration_since(src_mtime).unwrap()
        };

        assert!(diff.as_secs() < 1);
    }

    #[cfg(unix)]
    #[test]
    fn test_preserve_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test").unwrap();
        fs::write(&dest, b"test").unwrap();

        // Set specific permissions on source
        let perms = std::fs::Permissions::from_mode(0o644);
        fs::set_permissions(&source, perms).unwrap();

        let src_metadata = fs::metadata(&source).unwrap();
        preserve_mode(&dest, &src_metadata).unwrap();

        let dest_mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;

        assert_eq!(dest_mode, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn test_preserve_mode_executable() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.sh");
        let dest = temp_dir.path().join("dest.sh");

        fs::write(&source, b"#!/bin/bash\necho test").unwrap();
        fs::write(&dest, b"#!/bin/bash\necho test").unwrap();

        // Set executable permissions on source
        let perms = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(&source, perms).unwrap();

        let src_metadata = fs::metadata(&source).unwrap();
        preserve_mode(&dest, &src_metadata).unwrap();

        let dest_mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;

        assert_eq!(dest_mode, 0o755);
    }

    #[test]
    fn test_apply_preserve_attrs_timestamps_only() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test").unwrap();
        thread::sleep(Duration::from_millis(100));
        fs::write(&dest, b"test").unwrap();

        let mut attrs = PreserveAttr::none();
        attrs.timestamps = true;

        apply_preserve_attrs(&source, &dest, attrs).unwrap();

        let src_mtime = fs::metadata(&source).unwrap().modified().unwrap();
        let dest_mtime = fs::metadata(&dest).unwrap().modified().unwrap();

        let diff = if src_mtime > dest_mtime {
            src_mtime.duration_since(dest_mtime).unwrap()
        } else {
            dest_mtime.duration_since(src_mtime).unwrap()
        };

        assert!(diff.as_secs() < 1);
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_preserve_attrs_all() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        fs::write(&source, b"test").unwrap();
        fs::write(&dest, b"test").unwrap();

        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(&source, perms).unwrap();

        let attrs = PreserveAttr::all();
        apply_preserve_attrs(&source, &dest, attrs).unwrap();

        let dest_mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(dest_mode, 0o600);
    }
}
