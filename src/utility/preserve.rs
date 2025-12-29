use std::path::Path;
use tokio::io;

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

pub async fn apply_preserve_attrs(
    source: &Path,
    destination: &Path,
    attrs: PreserveAttr,
) -> io::Result<()> {
    let src_metadata = tokio::fs::metadata(source).await?;
    if attrs.timestamps {
        preserve_timestamps(destination, &src_metadata).await?;
    }
    #[cfg(unix)]
    if attrs.mode {
        preserve_mode(destination, &src_metadata).await?;
    }

    #[cfg(unix)]
    if attrs.ownership {
        preserve_ownership(destination, &src_metadata).await?;
    }
    Ok(())
}

async fn preserve_timestamps(
    destination: &Path,
    src_metadata: &std::fs::Metadata,
) -> io::Result<()> {
    use filetime::{FileTime, set_file_mtime};

    let modified_time = src_metadata
        .modified()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let system_modified_time = FileTime::from_system_time(modified_time);

    set_file_mtime(destination, system_modified_time)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}

#[cfg(unix)]
async fn preserve_mode(destination: &Path, src_metadata: &std::fs::Metadata) -> io::Result<()> {
    use std::fs::Permissions;

    let mode = src_metadata.permissions().mode();
    let permissions = Permissions::from_mode(mode);

    tokio::fs::set_permissions(destination, permissions).await?;

    Ok(())
}

#[cfg(unix)]
async fn preserve_ownership(
    destination: &Path,
    src_metadata: &std::fs::Metadata,
) -> io::Result<()> {
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

    #[tokio::test]
    async fn test_preserve_timestamps() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, b"test").await.unwrap();
        tokio::fs::write(&dest, b"test").await.unwrap();

        let src_metadata = tokio::fs::metadata(&source).await.unwrap();
        preserve_timestamps(&dest, &src_metadata).await.unwrap();

        let src_mtime = src_metadata.modified().unwrap();
        let dest_mtime = tokio::fs::metadata(&dest)
            .await
            .unwrap()
            .modified()
            .unwrap();

        // Allow for small differences due to precision
        let diff = if src_mtime > dest_mtime {
            src_mtime.duration_since(dest_mtime).unwrap()
        } else {
            dest_mtime.duration_since(src_mtime).unwrap()
        };

        assert!(diff.as_secs() < 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_preserve_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        tokio::fs::write(&source, b"test").await.unwrap();
        tokio::fs::write(&dest, b"test").await.unwrap();

        // Set specific permissions on source
        let perms = std::fs::Permissions::from_mode(0o644);
        tokio::fs::set_permissions(&source, perms).await.unwrap();

        let src_metadata = tokio::fs::metadata(&source).await.unwrap();
        preserve_mode(&dest, &src_metadata).await.unwrap();

        let dest_mode = tokio::fs::metadata(&dest)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(dest_mode, 0o644);
    }
}
