use std::path::{Path, PathBuf};
use tokio::io;
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
}

impl CopyPlan {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
            total_size: 0,
            total_files: 0,
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
}

pub async fn preprocess_file(source: &Path, destination: &Path) -> io::Result<CopyPlan> {
    let metadata = tokio::fs::metadata(source).await?;

    if metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("'{}' is a directory", source.display()),
        ));
    }

    let mut plan = CopyPlan::new();

    if let Ok(dest_meta) = tokio::fs::metadata(destination).await {
        if dest_meta.is_dir() {
            let file_name = source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?;
            let dest_path = destination.join(file_name);
            plan.add_file(source.to_path_buf(), dest_path, metadata.len());
            return Ok(plan);
        }
    }

    plan.add_file(
        source.to_path_buf(),
        destination.to_path_buf(),
        metadata.len(),
    );
    Ok(plan)
}

pub async fn preprocess_directory(source: &Path, destination: &Path) -> io::Result<CopyPlan> {
    let mut plan = CopyPlan::new();
    let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];

    while let Some((src_dir, dest_dir)) = stack.pop() {
        plan.add_directory(dest_dir.clone());
        let mut entries = tokio::fs::read_dir(&src_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());
            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                stack.push((src_path, dest_path));
            } else if metadata.is_file() {
                plan.add_file(src_path, dest_path, metadata.len());
            }
        }
    }
    Ok(plan)
}

pub async fn preprocess_multiple(sources: &[PathBuf], destination: &Path) -> io::Result<CopyPlan> {
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
        let file_name = source
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path"))?;
        let dest_path = destination.join(file_name);

        if metadata.is_dir() {
            let dir_plan = preprocess_directory(source, &dest_path).await?;
            plan.files.extend(dir_plan.files);
            plan.directories.extend(dir_plan.directories);
            plan.total_size += dir_plan.total_size;
            plan.total_files += dir_plan.total_files;
        } else {
            plan.add_file(source.clone(), dest_path, metadata.len());
        }
    }
    Ok(plan)
}
