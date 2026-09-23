use crate::cli::args::CopyOptions;
use crate::error::{CopyError, CopyResult};
use crate::utility::helper::create_with_mode;
use indicatif::ProgressBar;
use nix::fcntl::copy_file_range;
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;

/// Outcome of the copy_file_range fast path.
pub enum FastCopy {
    Done,
    /// copy_file_range is not usable here; the caller copies through
    /// userspace with these already-open files (reopening the destination
    /// could fail, e.g. it was just created read-only from a 0444 source).
    Fallback(std::fs::File, std::fs::File),
}

#[allow(clippy::too_many_arguments)]
pub fn fast_copy(
    source: &Path,
    destination: &Path,
    file_size: u64,
    file_mode: u32,
    maybe_sparse: bool,
    overall_pb: Option<&ProgressBar>,
    file_pb: Option<&ProgressBar>,
    options: &CopyOptions,
) -> CopyResult<FastCopy> {
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
    let dest_file = match create_with_mode(destination, file_mode) {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = std::fs::remove_file(destination).map_err(|e| CopyError::CopyFailed {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
                reason: format!("Failed to remove destination: {}", e),
            });
            create_with_mode(destination, file_mode).map_err(|e| CopyError::CopyFailed {
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

    if maybe_sparse
        && file_size > 0
        && dest_file.metadata().map(|m| m.is_file()).unwrap_or(false)
        && is_sparse(&src_file, file_size)
    {
        return match copy_sparse(
            &src_file, &dest_file, file_size, chunk_size, overall_pb, file_pb, options,
        )? {
            true => Ok(FastCopy::Done),
            false => Ok(rewind(src_file, dest_file)?),
        };
    }

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
                return Ok(rewind(src_file, dest_file)?);
            }
        }
    }
    Ok(FastCopy::Done)
}

/// Undo any partial fast-path progress so the userspace loop starts clean.
/// Best effort: a FIFO or device destination cannot be truncated or sought,
/// and nothing was written to it if copy_file_range refused it up front.
fn rewind(mut src_file: std::fs::File, mut dest_file: std::fs::File) -> io::Result<FastCopy> {
    use std::io::{Seek, SeekFrom};
    src_file.seek(SeekFrom::Start(0))?;
    let _ = dest_file.set_len(0);
    let _ = dest_file.seek(SeekFrom::Start(0));
    Ok(FastCopy::Fallback(src_file, dest_file))
}

fn lseek(file: &std::fs::File, offset: i64, whence: i32) -> Option<i64> {
    use std::os::fd::AsRawFd;
    // lseek64: off_t is 32 bits on some 32-bit glibc targets (armv7).
    let r = unsafe { libc::lseek64(file.as_raw_fd(), offset as libc::off64_t, whence) };
    (r >= 0).then_some(r as i64)
}

/// A file is sparse when its first hole starts before its end. The probe
/// moves the file offset, so it is reset for the offset-based copy loop.
fn is_sparse(src_file: &std::fs::File, file_size: u64) -> bool {
    let sparse =
        matches!(lseek(src_file, 0, libc::SEEK_HOLE), Some(hole) if (hole as u64) < file_size);
    lseek(src_file, 0, libc::SEEK_SET);
    sparse
}

/// Copy only the data extents (SEEK_DATA/SEEK_HOLE) so holes stay holes,
/// like GNU cp's --sparse=auto, then size the destination with ftruncate.
fn copy_sparse(
    src_file: &std::fs::File,
    dest_file: &std::fs::File,
    file_size: u64,
    chunk_size: usize,
    overall_pb: Option<&ProgressBar>,
    file_pb: Option<&ProgressBar>,
    options: &CopyOptions,
) -> CopyResult<bool> {
    let mut offset: i64 = 0;
    while (offset as u64) < file_size {
        let Some(data_start) = lseek(src_file, offset, libc::SEEK_DATA) else {
            break; // ENXIO: only holes remain
        };
        let data_end = lseek(src_file, data_start, libc::SEEK_HOLE).unwrap_or(file_size as i64);
        let skipped = (data_start - offset) as u64;
        if let Some(pb) = overall_pb {
            pb.inc(skipped);
        }
        if let Some(pb) = file_pb {
            pb.inc(skipped);
        }
        let mut pos_in = data_start;
        let mut pos_out = data_start;
        while pos_in < data_end {
            if options.abort.load(Ordering::Relaxed) {
                return Err(CopyError::Io(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Operation aborted by user",
                )));
            }
            let want = std::cmp::min(chunk_size as i64, data_end - pos_in) as usize;
            match copy_file_range(
                src_file,
                Some(&mut pos_in),
                dest_file,
                Some(&mut pos_out),
                want,
            ) {
                Ok(0) => break,
                Ok(copied) => {
                    if let Some(pb) = overall_pb {
                        pb.inc(copied as u64);
                    }
                    if let Some(pb) = file_pb {
                        pb.inc(copied as u64);
                    }
                }
                Err(_) => return Ok(false),
            }
        }
        offset = data_end;
    }
    dest_file.set_len(file_size)?;
    Ok(true)
}
