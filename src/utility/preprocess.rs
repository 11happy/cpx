use super::exclude::should_exclude;
use super::helper::with_parents;
use crate::cli::args::{CopyOptions, FollowSymlink, SymlinkMode};
use crate::error::{CopyError, CopyResult};
use jwalk::WalkDir;
use std::collections::HashMap;
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkKind {
    PreserveExact,
    RelativeToSource,
    AbsoluteToSource,
}

#[derive(Debug, Clone)]
pub struct FileTask {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub size: u64,
    pub inode_group: Option<u64>, // For tracking hard link groups
}

#[derive(Debug, Clone)]
pub struct DirectoryTask {
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SymlinkTask {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: SymlinkKind,
}

#[derive(Debug, Clone)]
pub struct HardlinkTask {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug)]
pub struct CopyPlan {
    pub files: Vec<FileTask>,
    pub directories: Vec<DirectoryTask>,
    pub symlinks: Vec<SymlinkTask>,
    pub hardlinks: Vec<HardlinkTask>,
    pub total_size: u64,
    pub total_files: usize,
    pub total_symlinks: usize,
    pub total_hardlinks: usize,
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
            symlinks: Vec::new(),
            hardlinks: Vec::new(),
            total_size: 0,
            total_files: 0,
            total_symlinks: 0,
            total_hardlinks: 0,
            skipped_files: 0,
            skipped_size: 0,
        }
    }

    pub fn add_file(&mut self, source: PathBuf, destination: PathBuf, size: u64) {
        self.add_file_with_inode(source, destination, size, None);
    }

    pub fn add_file_with_inode(
        &mut self,
        source: PathBuf,
        destination: PathBuf,
        size: u64,
        inode_group: Option<u64>,
    ) {
        self.files.push(FileTask {
            source,
            destination,
            size,
            inode_group,
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

    pub fn add_symlink(&mut self, source: PathBuf, destination: PathBuf, kind: SymlinkKind) {
        self.symlinks.push(SymlinkTask {
            source,
            destination,
            kind,
        });
        self.total_symlinks += 1;
    }

    pub fn add_hardlink(&mut self, source: PathBuf, destination: PathBuf) {
        self.hardlinks.push(HardlinkTask {
            source,
            destination,
        });
        self.total_hardlinks += 1;
    }

    pub fn mark_skipped(&mut self, size: u64) {
        self.skipped_files += 1;
        self.skipped_size += size;
    }

    pub fn sort_files_descending(&mut self) {
        self.files.sort_by(|a, b| b.size.cmp(&a.size));
    }

    pub fn merge(&mut self, other: CopyPlan) {
        self.files.extend(other.files);
        self.directories.extend(other.directories);
        self.symlinks.extend(other.symlinks);
        self.hardlinks.extend(other.hardlinks);
        self.total_size += other.total_size;
        self.total_files += other.total_files;
        self.total_symlinks += other.total_symlinks;
        self.total_hardlinks += other.total_hardlinks;
        self.skipped_files += other.skipped_files;
        self.skipped_size += other.skipped_size;
    }
}

fn symlink_kind_from_mode(source: &Path, mode: SymlinkMode) -> SymlinkKind {
    match mode {
        SymlinkMode::Absolute => SymlinkKind::AbsoluteToSource,
        SymlinkMode::Relative => SymlinkKind::RelativeToSource,
        SymlinkMode::Auto => {
            if source.is_absolute() {
                SymlinkKind::AbsoluteToSource
            } else {
                SymlinkKind::RelativeToSource
            }
        }
    }
}

fn calculate_checksum(path: &Path) -> io::Result<u64> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Xxh3::new();
    let mut buffer = vec![0u8; 128 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
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
        && src_modified <= dest_modified
    {
        return Ok(true);
    }

    let src_checksum = calculate_checksum(source)?;
    let dest_checksum = calculate_checksum(destination)?;

    Ok(src_checksum == dest_checksum)
}

fn process_entry(
    plan: &mut CopyPlan,
    source: &Path,
    source_root: &Path,
    dest_path: PathBuf,
    metadata: &Metadata,
    options: &CopyOptions,
    inode_groups: &mut Option<HashMap<u64, Vec<PathBuf>>>,
) -> io::Result<()> {
    if let Some(exclude_rules) = &options.exclude_rules
        && should_exclude(source, source_root, exclude_rules)
    {
        return Ok(());
    }

    // Handle hard link preservation
    let inode_group = if options.preserve.links && cfg!(unix) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let inode = metadata.ino();
            let nlink = metadata.nlink();

            // Only track if this is part of a hard link set (nlink > 1)
            if nlink > 1 {
                if inode_groups.is_none() {
                    *inode_groups = Some(HashMap::new());
                }

                let groups = inode_groups.as_mut().unwrap();
                let group_id = inode;

                groups.entry(group_id).or_default();
                groups.get_mut(&group_id).unwrap().push(dest_path.clone());

                Some(group_id)
            } else {
                None
            }
        }
        #[cfg(not(unix))]
        {
            None
        }
    } else {
        None
    };

    if metadata.file_type().is_symlink() {
        if !matches!(options.follow_symlink, FollowSymlink::Dereference) {
            if let Some(mode) = options.symbolic_link {
                let kind = symlink_kind_from_mode(source, mode);
                plan.add_symlink(source.to_path_buf(), dest_path, kind);
            } else {
                let original_target = std::fs::read_link(source)?;
                plan.add_symlink(original_target, dest_path, SymlinkKind::PreserveExact);
            }
        }
    } else if options.hard_link {
        plan.add_hardlink(source.to_path_buf(), dest_path);
    } else if let Some(mode) = options.symbolic_link {
        let kind = symlink_kind_from_mode(source, mode);
        plan.add_symlink(source.to_path_buf(), dest_path, kind);
    } else if options.resume && should_skip_file(source, &dest_path)? {
        plan.mark_skipped(metadata.len());
    } else {
        plan.add_file_with_inode(source.to_path_buf(), dest_path, metadata.len(), inode_group);
    }
    Ok(())
}

pub fn preprocess_file(
    source: &Path,
    source_root: &Path,
    destination: &Path,
    options: &CopyOptions,
    source_metadata: Metadata,
    destination_metadata: Option<Metadata>,
) -> CopyResult<CopyPlan> {
    if source_metadata.is_dir() {
        return Err(CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!("'{}' is a directory", source.display()),
        });
    }

    let mut plan = CopyPlan::new();

    let dest_path = if options.parents {
        let dest_meta = destination_metadata.ok_or_else(|| CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!(
                "Destination '{}' does not exist, with --parents destination must be a directory",
                destination.display()
            ),
        })?;

        if !dest_meta.is_dir() {
            return Err(CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!(
                    "Destination '{}' is not a directory, with --parents destination must be a directory",
                    destination.display()
                ),
            });
        }

        with_parents(destination, source)
    } else if let Some(dest_meta) = destination_metadata {
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

    if let Some(exclude_rules) = &options.exclude_rules
        && should_exclude(source, source_root, exclude_rules)
    {
        return Ok(plan);
    }
    if options.parents
        && let Some(parent) = dest_path.parent()
    {
        plan.add_directory(None, parent.to_path_buf());
    }

    let mut inode_groups = None;
    process_entry(
        &mut plan,
        source,
        source_root,
        dest_path.clone(),
        &source_metadata,
        options,
        &mut inode_groups,
    )
    .map_err(|e| CopyError::CopyFailed {
        source: source.to_path_buf(),
        destination: dest_path,
        reason: e.to_string(),
    })?;
    Ok(plan)
}

pub fn preprocess_directory(
    source: &Path,
    source_root: &Path,
    destination: &Path,
    options: &CopyOptions,
) -> CopyResult<CopyPlan> {
    let mut plan = CopyPlan::new();
    if source != source_root
        && let Some(exclude_rules) = &options.exclude_rules
        && should_exclude(source, source_root, exclude_rules)
    {
        return Ok(plan);
    }

    let root_destination =
        if options.parents {
            with_parents(destination, source)
        } else {
            destination.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?)
        };

    plan.add_directory(Some(source.into()), root_destination.clone());

    let num_threads = num_cpus::get().min(8);
    let follow_symlink = match options.follow_symlink {
        FollowSymlink::NoDereference | FollowSymlink::CommandLineSymlink => false,
        FollowSymlink::Dereference => true,
    };

    let walk_root = match options.follow_symlink {
        FollowSymlink::CommandLineSymlink => {
            let meta = std::fs::symlink_metadata(source)?;
            if meta.file_type().is_symlink() {
                std::fs::canonicalize(source).map_err(|e| CopyError::CopyFailed {
                    source: source.to_path_buf(),
                    destination: destination.to_path_buf(),
                    reason: format!("Failed to canonicalize symlink: {}", e),
                })?
            } else {
                source.to_path_buf()
            }
        }
        _ => source.to_path_buf(),
    };

    let mut inode_groups = None;

    for entry in WalkDir::new(&walk_root)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::RayonNewPool(num_threads))
        .follow_links(follow_symlink)
    {
        let entry = entry.map_err(|e| CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!("Failed to read directory entry: {}", e),
        })?;
        let src_path = entry.path();
        if src_path == walk_root {
            continue;
        }

        let relative = src_path
            .strip_prefix(&walk_root)
            .map_err(|_| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: "Failed to calculate relative path".to_string(),
            })?;

        let full_source_path = if walk_root != source {
            source.join(relative)
        } else {
            src_path.to_path_buf()
        };

        if let Some(exclude_rules) = &options.exclude_rules
            && should_exclude(&full_source_path, source, exclude_rules)
        {
            continue;
        }

        let dest_path = root_destination.join(relative);
        let metadata = entry.metadata().map_err(|e| CopyError::CopyFailed {
            source: src_path.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!("Failed to get metadata: {}", e),
        })?;

        if metadata.is_dir() {
            plan.add_directory(Some(src_path.to_path_buf()), dest_path);
        } else {
            process_entry(
                &mut plan,
                &src_path,
                &walk_root,
                dest_path,
                &metadata,
                options,
                &mut inode_groups,
            )?;
        }
    }

    plan.sort_files_descending();
    Ok(plan)
}

pub fn preprocess_multiple(
    sources: &[PathBuf],
    destination: &Path,
    options: &CopyOptions,
) -> CopyResult<CopyPlan> {
    let dest_metadata = std::fs::metadata(destination)
        .map_err(|_e| CopyError::InvalidDestination(destination.to_path_buf()))?;
    if !dest_metadata.is_dir() {
        return Err(CopyError::CopyFailed {
            source: PathBuf::new(),
            destination: destination.to_path_buf(),
            reason: format!("Destination '{}' is not a directory", destination.display()),
        });
    }

    let mut plan = CopyPlan::new();

    for source in sources {
        let metadata = match options.follow_symlink {
            FollowSymlink::Dereference | FollowSymlink::CommandLineSymlink => {
                std::fs::metadata(source)
                    .map_err(|_e| CopyError::InvalidSource(source.to_path_buf()))?
            }
            FollowSymlink::NoDereference => std::fs::symlink_metadata(source)
                .map_err(|_e| CopyError::InvalidSource(source.to_path_buf()))?,
        };

        if metadata.is_dir() {
            let dir_plan =
                preprocess_directory(source, source, destination, options).map_err(|e| {
                    CopyError::CopyFailed {
                        source: source.to_path_buf(),
                        destination: destination.to_path_buf(),
                        reason: e.to_string(),
                    }
                })?;
            plan.merge(dir_plan);
        } else {
            let _source_root = source.parent().unwrap_or_else(|| Path::new("."));

            let dest_path = if options.parents {
                with_parents(destination, source)
            } else {
                destination.join(source.file_name().ok_or_else(|| CopyError::CopyFailed {
                    source: source.to_path_buf(),
                    destination: destination.to_path_buf(),
                    reason: "Invalid source path".to_string(),
                })?)
            };

            if options.parents
                && let Some(parent) = dest_path.parent()
            {
                plan.add_directory(None, parent.to_path_buf());
            }

            let mut inode_groups = None;
            process_entry(
                &mut plan,
                source,
                source,
                dest_path.clone(),
                &metadata,
                options,
                &mut inode_groups,
            )
            .map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: dest_path.clone(),
                reason: e.to_string(),
            })?;
        }
    }

    plan.sort_files_descending();
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
        let options = CopyOptions::none();
        let plan = preprocess_directory(&source_dir, &source_dir, &dest_dir, &options).unwrap();

        assert_eq!(plan.total_files, 3);
        assert!(!plan.directories.is_empty());
    }

    #[test]
    fn test_preprocess_file_with_symlink_auto() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");

        create_test_file(&source, b"content").unwrap();
        std_fs::create_dir(&dest_dir).unwrap();

        let source_metadata = std_fs::metadata(&source).unwrap();
        let dest_metadata = Some(std_fs::metadata(&dest_dir).unwrap());

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Auto);

        let plan = preprocess_file(
            &source,
            source.parent().unwrap_or(Path::new(".")),
            &dest_dir,
            &options,
            source_metadata,
            dest_metadata,
        )
        .unwrap();

        assert_eq!(plan.total_files, 0);
        assert_eq!(plan.total_symlinks, 1);
        assert_eq!(plan.symlinks.len(), 1);

        let symlink = &plan.symlinks[0];
        assert_eq!(symlink.source, source);
    }

    #[test]
    fn test_preprocess_file_with_symlink_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");

        create_test_file(&source, b"content").unwrap();
        std_fs::create_dir(&dest_dir).unwrap();

        let source_metadata = std_fs::metadata(&source).unwrap();
        let dest_metadata = Some(std_fs::metadata(&dest_dir).unwrap());

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Absolute);

        let plan = preprocess_file(
            &source,
            source.parent().unwrap_or(Path::new(".")),
            &dest_dir,
            &options,
            source_metadata,
            dest_metadata,
        )
        .unwrap();

        assert_eq!(plan.total_symlinks, 1);
    }

    #[test]
    fn test_preprocess_file_with_symlink_relative() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");

        create_test_file(&source, b"content").unwrap();
        std_fs::create_dir(&dest_dir).unwrap();

        let source_metadata = std_fs::metadata(&source).unwrap();
        let dest_metadata = Some(std_fs::metadata(&dest_dir).unwrap());

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Relative);

        let plan = preprocess_file(
            &source,
            source.parent().unwrap_or(Path::new(".")),
            &dest_dir,
            &options,
            source_metadata,
            dest_metadata,
        )
        .unwrap();

        assert_eq!(plan.total_symlinks, 1);
    }

    #[test]
    fn test_preprocess_directory_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        std_fs::create_dir_all(&source_dir).unwrap();
        create_test_file(&source_dir.join("file1.txt"), b"content1").unwrap();
        create_test_file(&source_dir.join("file2.txt"), b"content2").unwrap();

        let subdir = source_dir.join("subdir");
        std_fs::create_dir_all(&subdir).unwrap();
        create_test_file(&subdir.join("file3.txt"), b"content3").unwrap();

        let mut options = CopyOptions::none();
        options.recursive = true;
        options.symbolic_link = Some(SymlinkMode::Auto);

        let plan = preprocess_directory(&source_dir, &source_dir, &dest_dir, &options).unwrap();

        assert_eq!(plan.total_files, 0);
        assert_eq!(plan.total_symlinks, 3);
        assert!(!plan.directories.is_empty());
        assert_eq!(plan.symlinks.len(), 3);
    }

    #[test]
    fn test_preprocess_multiple_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let dest_dir = temp_dir.path().join("dest");
        std_fs::create_dir(&dest_dir).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        create_test_file(&file1, b"content1").unwrap();
        create_test_file(&file2, b"content2").unwrap();

        let sources = vec![file1.clone(), file2.clone()];

        let mut options = CopyOptions::none();
        options.symbolic_link = Some(SymlinkMode::Relative);

        let plan = preprocess_multiple(&sources, &dest_dir, &options).unwrap();

        assert_eq!(plan.total_files, 0);
        assert_eq!(plan.total_symlinks, 2);
        assert_eq!(plan.symlinks.len(), 2);
    }

    #[test]
    fn test_preprocess_file_normal_copy_mode() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");

        create_test_file(&source, b"content").unwrap();
        std_fs::create_dir(&dest_dir).unwrap();

        let source_metadata = std_fs::metadata(&source).unwrap();
        let dest_metadata = Some(std_fs::metadata(&dest_dir).unwrap());

        let options = CopyOptions::none(); // No symlink mode

        let plan = preprocess_file(
            &source,
            source.parent().unwrap_or(Path::new(".")),
            &dest_dir,
            &options,
            source_metadata,
            dest_metadata,
        )
        .unwrap();

        assert_eq!(plan.total_files, 1);
        assert_eq!(plan.total_symlinks, 0);
        assert!(plan.symlinks.is_empty());
    }

    #[test]
    fn test_copy_plan_add_symlink() {
        let mut plan = CopyPlan::new();
        let source = PathBuf::from("/source/file.txt");
        let dest = PathBuf::from("/dest/file.txt");

        plan.add_symlink(source.clone(), dest.clone(), SymlinkKind::AbsoluteToSource);

        assert_eq!(plan.total_symlinks, 1);
        assert_eq!(plan.symlinks.len(), 1);
        assert_eq!(plan.symlinks[0].source, source);
        assert_eq!(plan.symlinks[0].destination, dest);
    }
}
