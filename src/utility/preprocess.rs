use super::helper::with_parents;
use jwalk::WalkDir;
use std::io;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Clone)]
pub struct FileTask {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct DirectoryTask {
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
}

#[derive(Debug)]
pub struct CopyPlan {
    pub files: Vec<FileTask>,
    pub directories: Vec<DirectoryTask>,
    pub total_size: u64,
    pub total_files: usize,
    pub skipped_files: usize,
    pub skipped_size: u64,
}

impl Default for CopyPlan {
    fn default() -> Self {
        Self::new()
    }
}

impl CopyPlan {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
            total_size: 0,
            total_files: 0,
            skipped_files: 0,
            skipped_size: 0,
        }
    }
    pub fn add_file(&mut self, source: PathBuf, destination: PathBuf, size: u64) {
        self.files.push(FileTask {
            source,
            destination,
            size,
        });
        self.total_size += size;
        self.total_files += 1;
    }

    pub fn add_directory(&mut self, source: Option<PathBuf>, destination: PathBuf) {
        self.directories.push(DirectoryTask {
            source,
            destination,
        });
    }

    pub fn mark_skipped(&mut self, size: u64) {
        self.skipped_files += 1;
        self.skipped_size += size;
    }

    pub fn sort_by_size_desc(&mut self) {
        self.files.sort_by(|a, b| b.size.cmp(&a.size));
    }
}

fn calculate_checksum(path: &Path) -> io::Result<u64> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Xxh3::new(); // streaming xxh3 hasher
    let mut buffer = vec![0u8; 128 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]); // no RAM growth
    }

    Ok(hasher.digest())
}

pub fn should_skip_file(source: &Path, destination: &Path) -> io::Result<bool> {
    let dest_metadata = match std::fs::metadata(destination) {
        Ok(meta) => meta,
        Err(_) => return Ok(false),
    };

    let src_metadata = std::fs::metadata(source)?;

    if dest_metadata.len() != src_metadata.len() {
        return Ok(false);
    }

    if let (Ok(src_modified), Ok(dest_modified)) =
        (src_metadata.modified(), dest_metadata.modified())
        && src_modified < dest_modified
    {
        return Ok(true);
    }

    let src_checksum = calculate_checksum(source)?;
    let dest_checksum = calculate_checksum(destination)?;

    Ok(src_checksum == dest_checksum)
}

pub fn preprocess_file(
    source: &Path,
    destination: &Path,
    resume: bool,
    parents: bool,
) -> io::Result<CopyPlan> {
    let src_metadata = std::fs::metadata(source)?;

    if src_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("'{}' is a directory", source.display()),
        ));
    }

    let mut plan = CopyPlan::new();

    let dest_path = if parents {
        let dest_meta = std::fs::metadata(destination).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Destination '{}' does not exist, with --parents destination must be a directory",
                destination.display()
            ),
        )
    })?;

        if !dest_meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Destination '{}' is not a directory, with --parents destination must be a directory",
                    destination.display()
                ),
            ));
        }

        with_parents(destination, source)
    } else if let Ok(dest_meta) = std::fs::metadata(destination) {
        if dest_meta.is_dir() {
            destination.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?)
        } else {
            destination.to_path_buf()
        }
    } else {
        destination.to_path_buf()
    };
    if parents && let Some(parent) = dest_path.parent() {
        plan.add_directory(None, parent.to_path_buf());
    }
    if resume && should_skip_file(source, &dest_path)? {
        plan.mark_skipped(src_metadata.len());
    } else {
        plan.add_file(source.to_path_buf(), dest_path, src_metadata.len());
    }
    Ok(plan)
}

pub fn preprocess_directory(
    source: &Path,
    destination: &Path,
    resume: bool,
    parents: bool,
) -> io::Result<CopyPlan> {
    let mut plan = CopyPlan::new();
    let root_destination =
        if parents {
            with_parents(destination, source)
        } else {
            destination.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?)
        };
    plan.add_directory(Some(source.into()), root_destination.clone());
    let num_threads = num_cpus::get().min(8);
    for entry in WalkDir::new(source)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::RayonNewPool(num_threads))
    {
        let entry = entry?;
        let src_path = entry.path();

        if src_path == source {
            continue;
        }
        let relative = src_path.strip_prefix(source).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Failed to calculate relative path",
            )
        })?;
        let dest_path = root_destination.join(relative);
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            plan.add_directory(Some(src_path.to_path_buf()), dest_path);
        } else if metadata.is_file() {
            if resume && should_skip_file(&src_path, &dest_path)? {
                plan.mark_skipped(metadata.len());
            } else {
                plan.add_file(src_path.to_path_buf(), dest_path, metadata.len());
            }
        }
    }
    plan.sort_by_size_desc();
    Ok(plan)
}

pub fn preprocess_multiple(
    sources: &[PathBuf],
    destination: &Path,
    resume: bool,
    parents: bool,
) -> io::Result<CopyPlan> {
    let dest_metadata = std::fs::metadata(destination)?;
    if !dest_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Destination '{}' is not a directory", destination.display()),
        ));
    }

    let mut plan = CopyPlan::new();
    for source in sources {
        let metadata = std::fs::metadata(source)?;

        if metadata.is_dir() {
            let dir_plan = preprocess_directory(source, destination, resume, parents)?;
            plan.files.extend(dir_plan.files);
            plan.directories.extend(dir_plan.directories);
            plan.total_size += dir_plan.total_size;
            plan.total_files += dir_plan.total_files;
            plan.skipped_files += dir_plan.skipped_files;
            plan.skipped_size += dir_plan.skipped_size;
        } else {
            let file_name = source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?;

            let dest_path = if parents {
                with_parents(destination, source)
            } else {
                destination.join(file_name)
            };

            if parents && let Some(parent) = dest_path.parent() {
                plan.add_directory(None, parent.to_path_buf());
            }

            if resume && should_skip_file(source, &dest_path)? {
                plan.mark_skipped(metadata.len());
            } else {
                plan.add_file(source.clone(), dest_path, metadata.len());
            }
        }
    }
    plan.sort_by_size_desc();
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as std_fs;
    use tempfile::TempDir;

    fn create_test_file(path: &Path, content: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std_fs::create_dir_all(parent)?;
        }
        std_fs::write(path, content)
    }

    #[test]
    fn test_calculate_checksum_same_content() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        let content = b"Hello, World!";
        create_test_file(&file1, content).unwrap();
        create_test_file(&file2, content).unwrap();

        let hash1 = calculate_checksum(&file1).unwrap();
        let hash2 = calculate_checksum(&file2).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_preprocess_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        std_fs::create_dir_all(&source_dir).unwrap();
        create_test_file(&source_dir.join("file1.txt"), b"content1").unwrap();
        create_test_file(&source_dir.join("file2.txt"), b"content2").unwrap();

        let subdir = source_dir.join("subdir");
        std_fs::create_dir_all(&subdir).unwrap();
        create_test_file(&subdir.join("file3.txt"), b"content3").unwrap();

        let plan = preprocess_directory(&source_dir, &dest_dir, false, false).unwrap();

        assert_eq!(plan.total_files, 3);
        assert!(!plan.directories.is_empty());
    }
}
