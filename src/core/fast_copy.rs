use crate::cli::args::CopyOptions;
use crate::error::{CopyError, CopyResult};
use indicatif::ProgressBar;
use nix::fcntl::copy_file_range;
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;

pub fn fast_copy(
    source: &Path,
    destination: &Path,
    file_size: u64,
    overall_pb: Option<&ProgressBar>,
    file_pb: Option<&ProgressBar>,
    options: &CopyOptions,
) -> CopyResult<bool> {
    let src_file = std::fs::File::open(source).map_err(|e| CopyError::CopyFailed {
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        reason: format!("Failed to open source file: {}", e),
    })?;
    if options.remove_destination {
        let exists = std::fs::exists(destination).unwrap_or(false);

        if exists {
            std::fs::remove_file(destination).map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!("Failed to remove destination: {}", e),
            })?;
        }
    }
    let dest_file = match std::fs::File::create(destination) {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = std::fs::remove_file(destination).map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!("Failed to remove destination: {}", e),
            });
            std::fs::File::create(destination).map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!("Failed to create destination: {}", e),
            })?
        }
        Err(e) => return Err(CopyError::from(e)),
    };
    const TARGET_UPDATES: u64 = 128;
    const MIN_CHUNK: usize = 4 * 1024 * 1024;
    let chunk_size = std::cmp::max(MIN_CHUNK, (file_size / TARGET_UPDATES) as usize);
    let mut total_copied = 0u64;
    loop {
        if options.abort.load(Ordering::Relaxed) {
            drop(dest_file); // Close file
            if let Err(e) = std::fs::remove_file(destination) {
                eprintln!(
                    "Could not remove incomplete file {}: {}",
                    destination.display(),
                    e
                );
            } else {
                eprintln!("Cleaned up incomplete file: {}", destination.display());
            }
            return Err(CopyError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted by user",
            )));
        }

        // Don't bound the request by file_size: procfs/sysfs files report
        // st_size 0 but still have content. Stop once the known size has been
        // copied so a regular file costs one syscall, not one plus an EOF probe.
        match copy_file_range(&src_file, None, &dest_file, None, chunk_size) {
            Ok(0) => break,
            Ok(copied) => {
                total_copied += copied as u64;
                if let Some(pb) = overall_pb {
                    pb.inc(copied as u64);
                }
                if let Some(pb) = file_pb {
                    pb.inc(copied as u64);
                }
                if file_size > 0 && total_copied >= file_size {
                    break;
                }
            }
            Err(_) => {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
