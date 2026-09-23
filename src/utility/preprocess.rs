use super::exclude::should_exclude;
use super::helper::with_parents;
use crate::cli::args::{CopyOptions, FollowSymlink, SymlinkMode};
use crate::error::{CopyError, CopyResult};
use jwalk::WalkDirGeneric;
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
    /// Source permission bits; the destination is created with them so a
    /// new file gets `mode & !umask` like GNU cp, even without --preserve.
    pub mode: u32,
    /// st_blocks says the file may have holes; only then is the SEEK_HOLE
    /// probe paid for at copy time.
    pub sparse: bool,
    /// (device, inode) of the source when --preserve=links is on, so files
    /// that are hard links of each other can be re-linked in the copy.
    pub inode_group: Option<(u64, u64)>,
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

#[derive(Debug, Clone)]
pub struct FifoTask {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub mode: u32,
}

#[derive(Debug)]
pub struct CopyPlan {
    pub files: Vec<FileTask>,
    pub directories: Vec<DirectoryTask>,
    pub symlinks: Vec<SymlinkTask>,
    pub hardlinks: Vec<HardlinkTask>,
    /// --preserve=links: later occurrences of an inode already planned as a
    /// file copy; created as hard links to that copy after the files are done.
    pub preserved_links: Vec<HardlinkTask>,
    pub fifos: Vec<FifoTask>,
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
            preserved_links: Vec::new(),
            fifos: Vec::new(),
            total_size: 0,
            total_files: 0,
            total_symlinks: 0,
            total_hardlinks: 0,
            skipped_files: 0,
            skipped_size: 0,
        }
    }

    pub fn add_file(&mut self, source: PathBuf, destination: PathBuf, size: u64) {
        self.add_file_with_inode(source, destination, size, 0o666, false, None);
    }

    /// Register `dest/a`, `dest/a/b`, ... for `--parents`, each paired with
    /// the matching source ancestor so its attributes can be preserved.
    pub fn add_parent_directories(&mut self, destination: &Path, source: &Path) {
        let skip = if source.is_absolute() { 1 } else { 0 };
        let mut src_ancestor = if source.is_absolute() {
            PathBuf::from("/")
        } else {
            PathBuf::new()
        };
        let mut dest_ancestor = destination.to_path_buf();
        let comps: Vec<_> = source.components().skip(skip).collect();
        for comp in comps.iter().take(comps.len().saturating_sub(1)) {
            src_ancestor.push(comp.as_os_str());
            dest_ancestor.push(comp.as_os_str());
            self.add_directory(Some(src_ancestor.clone()), dest_ancestor.clone());
        }
    }

    /// Refuse plans where two sources map to the same destination, like GNU
    /// cp's "will not overwrite just-created ... with ...". Sorting once is
    /// O(n log n); the previous per-task linear scan made planning O(n²).
    pub fn check_collisions(&self) -> CopyResult<()> {
        let mut dests: Vec<(&Path, &Path)> = self
            .files
            .iter()
            .map(|t| (t.destination.as_path(), t.source.as_path()))
            .chain(
                self.symlinks
                    .iter()
                    .map(|t| (t.destination.as_path(), t.source.as_path())),
            )
            .chain(
                self.hardlinks
                    .iter()
                    .map(|t| (t.destination.as_path(), t.source.as_path())),
            )
            .collect();
        dests.sort_unstable();
        for pair in dests.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(CopyError::CopyFailed {
                    source: pair[1].1.to_path_buf(),
                    destination: pair[1].0.to_path_buf(),
                    reason: format!(
                        "will not overwrite just-created '{}' with '{}'",
                        pair[1].0.display(),
                        pair[1].1.display()
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn add_file_with_inode(
        &mut self,
        source: PathBuf,
        destination: PathBuf,
        size: u64,
        mode: u32,
        sparse: bool,
        inode_group: Option<(u64, u64)>,
    ) {
        self.files.push(FileTask {
            source,
            destination,
            size,
            mode,
            sparse,
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

    /// Turn every file whose (device, inode) was already planned into a hard
    /// link of that first copy. Done once on the complete plan, so the result
    /// is deterministic regardless of how the parallel copy is scheduled.
    pub fn link_duplicate_inodes(&mut self) {
        let mut first_dest: HashMap<(u64, u64), PathBuf> = HashMap::new();
        let files = std::mem::take(&mut self.files);
        for task in files {
            match task.inode_group {
                Some(key) => match first_dest.get(&key) {
                    Some(first) => {
                        self.total_files -= 1;
                        self.total_size -= task.size;
                        self.preserved_links.push(HardlinkTask {
                            source: first.clone(),
                            destination: task.destination,
                        });
                    }
                    None => {
                        first_dest.insert(key, task.destination.clone());
                        self.files.push(task);
                    }
                },
                None => self.files.push(task),
            }
        }
    }

    pub fn mark_skipped(&mut self, size: u64) {
        self.skipped_files += 1;
        self.skipped_size += size;
    }

    pub fn sort_files_descending(&mut self) {
        self.files.sort_by_key(|f| std::cmp::Reverse(f.size));
    }

    pub fn merge(&mut self, other: CopyPlan) {
        self.files.extend(other.files);
        self.directories.extend(other.directories);
        self.symlinks.extend(other.symlinks);
        self.hardlinks.extend(other.hardlinks);
        self.preserved_links.extend(other.preserved_links);
        self.fifos.extend(other.fifos);
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

/// `-u/--update`: the destination exists and is at least as new as the source.
fn dest_is_up_to_date(destination: &Path, src_metadata: &Metadata) -> bool {
    match (
        std::fs::metadata(destination).and_then(|m| m.modified()),
        src_metadata.modified(),
    ) {
        (Ok(dest_modified), Ok(src_modified)) => src_modified <= dest_modified,
        _ => false,
    }
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
) -> io::Result<()> {
    if let Some(exclude_rules) = &options.exclude_rules
        && should_exclude(source, source_root, exclude_rules)
    {
        return Ok(());
    }

    // Handle hard link preservation (nlink is not checked: with -L two
    // symlinks to one file must also become one inode in the copy).
    #[cfg(unix)]
    let inode_group = if options.preserve.links {
        use std::os::unix::fs::MetadataExt;
        Some((metadata.dev(), metadata.ino()))
    } else {
        None
    };
    #[cfg(not(unix))]
    let inode_group = None;

    #[cfg(unix)]
    if options.recursive && !metadata.file_type().is_symlink() {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};
        let file_type = metadata.file_type();
        if file_type.is_fifo() {
            plan.fifos.push(FifoTask {
                source: source.to_path_buf(),
                destination: dest_path,
                mode: metadata.permissions().mode() & 0o7777,
            });
            return Ok(());
        }
        if file_type.is_socket() || file_type.is_char_device() || file_type.is_block_device() {
            // Opening these would block or read device data; GNU cp recreates
            // them with mknod, which needs root. Skip rather than hang.
            eprintln!(
                "cpx: skipping special file '{}' (not a regular file)",
                source.display()
            );
            return Ok(());
        }
    }

    if metadata.file_type().is_symlink() {
        if !matches!(options.follow_symlink, FollowSymlink::Dereference) {
            if options.hard_link {
                // GNU cp -l -P hard-links the symlink itself.
                plan.add_hardlink(source.to_path_buf(), dest_path);
            } else if let Some(mode) = options.symbolic_link {
                let kind = symlink_kind_from_mode(source, mode);
                plan.add_symlink(source.to_path_buf(), dest_path, kind);
            } else {
                plan.add_symlink(source.to_path_buf(), dest_path, SymlinkKind::PreserveExact);
            }
        }
    } else if options.hard_link {
        // link(2) never follows symlinks; with -L/-H link the resolved target.
        let link_source = if !matches!(options.follow_symlink, FollowSymlink::NoDereference)
            && source.is_symlink()
        {
            source.canonicalize()?
        } else {
            source.to_path_buf()
        };
        plan.add_hardlink(link_source, dest_path);
    } else if let Some(mode) = options.symbolic_link {
        let kind = symlink_kind_from_mode(source, mode);
        plan.add_symlink(source.to_path_buf(), dest_path, kind);
    } else if options.resume && should_skip_file(source, &dest_path)? {
        plan.mark_skipped(metadata.len());
    } else if options.update && dest_is_up_to_date(&dest_path, metadata) {
        // GNU cp -u silently leaves up-to-date destinations alone.
    } else if options.no_clobber && dest_path.symlink_metadata().is_ok() {
        // GNU cp -n silently leaves existing destinations alone.
    } else {
        #[cfg(unix)]
        let (mode, sparse) = {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            (
                metadata.permissions().mode() & 0o7777,
                metadata.blocks() * 512 < metadata.len(),
            )
        };
        #[cfg(not(unix))]
        let (mode, sparse) = (0o666, false);
        plan.add_file_with_inode(
            source.to_path_buf(),
            dest_path,
            metadata.len(),
            mode,
            sparse,
            inode_group,
        );
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
    if options.parents {
        plan.add_parent_directories(destination, source);
    }

    process_entry(
        &mut plan,
        source,
        source_root,
        dest_path.clone(),
        &source_metadata,
        options,
    )
    .map_err(|e| CopyError::CopyFailed {
        source: source.to_path_buf(),
        destination: dest_path,
        reason: e.to_string(),
    })?;
    Ok(plan)
}

/// Canonicalize a path that may not exist yet: resolve the nearest existing
/// ancestor and re-append the missing tail.
fn canonicalize_missing(path: &Path) -> Option<PathBuf> {
    let mut tail = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(canon) = cur.canonicalize() {
            let mut result = canon;
            for comp in tail.iter().rev() {
                result.push(comp);
            }
            return Some(result);
        }
        let name = cur.file_name()?.to_os_string();
        tail.push(name);
        cur = cur.parent()?.to_path_buf();
        if cur.as_os_str().is_empty() {
            cur = PathBuf::from(".");
        }
    }
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

    // `cpx -r dir/. dest` and `cpx -r . dest` copy the directory's contents
    // into dest itself, as with GNU cp.
    let source_is_dot = matches!(
        source.components().next_back(),
        Some(std::path::Component::CurDir)
    ) || source.as_os_str().as_encoded_bytes().ends_with(b"/.");
    let root_destination =
        if options.parents {
            with_parents(destination, source)
        } else if source_is_dot {
            destination.to_path_buf()
        } else {
            destination.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source path")
            })?)
        };

    // `cpx -r dir dir` or `cpx -r dir dir/sub` would recurse into its own copy.
    if let (Ok(src_canon), Some(dest_canon)) = (
        source.canonicalize(),
        canonicalize_missing(&root_destination),
    ) && dest_canon.starts_with(&src_canon)
    {
        return Err(CopyError::CopyFailed {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            reason: format!(
                "cannot copy a directory, '{}', into itself, '{}'",
                source.display(),
                root_destination.display()
            ),
        });
    }

    if options.parents {
        plan.add_parent_directories(destination, source);
    }
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

    // Stat entries and apply exclude rules inside the walker's per-directory
    // callback: it runs on the walker's thread pool, and excluded directories
    // are pruned before they are descended into.
    let exclude_rules = options.exclude_rules.clone();
    let exclude_source = source.to_path_buf();
    let exclude_walk_root = walk_root.clone();
    let walker = WalkDirGeneric::<((), Option<Metadata>)>::new(&walk_root)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::RayonNewPool(num_threads))
        .follow_links(follow_symlink)
        .process_read_dir(move |_depth, _path, _state, children| {
            if let Some(rules) = &exclude_rules {
                children.retain(|child| match child {
                    Ok(child) => {
                        let path = child.path();
                        let full_source_path = match path.strip_prefix(&exclude_walk_root) {
                            Ok(relative) if exclude_walk_root != exclude_source => {
                                exclude_source.join(relative)
                            }
                            _ => path,
                        };
                        !should_exclude(&full_source_path, &exclude_source, rules)
                    }
                    Err(_) => true,
                });
            }
            for child in children.iter_mut().flatten() {
                child.client_state = child.metadata().ok();
            }
        });

    for entry in walker {
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

        let dest_path = root_destination.join(relative);
        let metadata = match entry.client_state {
            Some(ref metadata) => metadata.clone(),
            None => entry.metadata().map_err(|e| CopyError::CopyFailed {
                source: src_path.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!("Failed to get metadata: {}", e),
            })?,
        };

        if metadata.is_dir() {
            plan.add_directory(Some(src_path.to_path_buf()), dest_path);
        } else {
            process_entry(
                &mut plan, &src_path, &walk_root, dest_path, &metadata, options,
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

            if options.parents {
                plan.add_parent_directories(destination, source);
            }

            process_entry(
                &mut plan,
                source,
                source,
                dest_path.clone(),
                &metadata,
                options,
            )
            .map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: dest_path.clone(),
                reason: e.to_string(),
            })?;
        }
    }

    plan.sort_files_descending();
    plan.check_collisions()?;
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
