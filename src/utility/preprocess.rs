use super::helper::with_parents;
use std::path::{Path, PathBuf};
use tokio::io;
use tokio::io::AsyncReadExt;
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Clone)]
pub struct FileTask {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub size: u64,
}

#[derive(Debug)]
pub struct CopyPlan {
    pub files: Vec<FileTask>,
    pub directories: Vec<PathBuf>,
    pub total_size: u64,
    pub total_files: usize,
    pub skipped_files: usize,
    pub skipped_size: u64,
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

    pub fn add_directory(&mut self, path: PathBuf) {
        self.directories.push(path);
    }

    pub fn mark_skipped(&mut self, size: u64) {
        self.skipped_files += 1;
        self.skipped_size += size;
    }

    pub fn sort_by_size_desc(&mut self) {
        self.files.sort_by(|a, b| b.size.cmp(&a.size));
    }
}

pub async fn calculate_checksum(path: &Path) -> io::Result<u64> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Xxh3::new(); // streaming xxh3 hasher
    let mut buffer = vec![0u8; 128 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]); // no RAM growth
    }

    Ok(hasher.digest())
}

async fn should_skip_file(source: &Path, destination: &Path) -> io::Result<bool> {
    let dest_metadata = match tokio::fs::metadata(destination).await {
        Ok(meta) => meta,
        Err(_) => return Ok(false),
    };

    let src_metadata = tokio::fs::metadata(source).await?;

    if dest_metadata.len() != src_metadata.len() {
        return Ok(false);
    }

    if let (Ok(src_modified), Ok(dest_modified)) =
        (src_metadata.modified(), dest_metadata.modified())
    {
        if src_modified < dest_modified {
            return Ok(true);
        }
    }

    let src_checksum = calculate_checksum(source).await?;
    let dest_checksum = calculate_checksum(destination).await?;

    Ok(src_checksum == dest_checksum)
}

pub async fn preprocess_file(
    source: &Path,
    destination: &Path,
    resume: bool,
    parents: bool,
) -> io::Result<CopyPlan> {
    let metadata = tokio::fs::metadata(source).await?;

    if metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("'{}' is a directory", source.display()),
        ));
    }

    let mut plan = CopyPlan::new();

    let dest_path = if parents {
        let dest_meta = tokio::fs::metadata(destination).await.map_err(|_| {
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
    } else if let Ok(dest_meta) = tokio::fs::metadata(destination).await {
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
        plan.add_directory(parent.to_path_buf());
    }
    if resume && should_skip_file(source, &dest_path).await? {
        plan.mark_skipped(metadata.len());
    } else {
        plan.add_file(source.to_path_buf(), dest_path, metadata.len());
    }
    Ok(plan)
}

pub async fn preprocess_directory(
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
    let mut stack = vec![(source.to_path_buf(), root_destination)];

    while let Some((src_dir, dest_dir)) = stack.pop() {
        plan.add_directory(dest_dir.clone());
        let mut entries = tokio::fs::read_dir(&src_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());
            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                stack.push((src_path, dest_path));
            } else {
                if resume && should_skip_file(&src_path, &dest_path).await? {
                    plan.mark_skipped(metadata.len());
                } else {
                    plan.add_file(src_path, dest_path, metadata.len());
                }
            }
        }
    }
    plan.sort_by_size_desc();
    Ok(plan)
}

pub async fn preprocess_multiple(
    sources: &[PathBuf],
    destination: &Path,
    resume: bool,
    parents: bool,
) -> io::Result<CopyPlan> {
    let dest_metadata = tokio::fs::metadata(destination).await?;
    if !dest_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Destination '{}' is not a directory", destination.display()),
        ));
    }

    let mut plan = CopyPlan::new();
    for source in sources {
        let metadata = tokio::fs::metadata(source).await?;

        if metadata.is_dir() {
            let dir_plan = preprocess_directory(source, destination, resume, parents).await?;
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
                plan.add_directory(parent.to_path_buf());
            }

            if resume && should_skip_file(source, &dest_path).await? {
                plan.mark_skipped(metadata.len());
            } else {
                plan.add_file(source.clone(), dest_path, metadata.len());
            }
        }
    }
    plan.sort_by_size_desc();
    Ok(plan)
}
