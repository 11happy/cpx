use indicatif::ProgressBar;
use nix::fcntl::copy_file_range;
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;

use crate::cli::args::CopyOptions;

pub fn fast_copy(
    source: &Path,
    destination: &Path,
    file_size: u64,
    overall_pb: Option<&ProgressBar>,
    options: &CopyOptions,
) -> io::Result<bool> {
    let src_file = std::fs::File::open(source)?;
    if options.remove_destination {
        let exists = std::fs::exists(destination).unwrap_or(false);

        if exists {
            std::fs::remove_file(destination)?;
        }
    }
    let dest_file = match std::fs::File::create(destination) {
        Ok(file) => file,
        Err(_e) if options.force => {
            let _ = std::fs::remove_file(destination);
            std::fs::File::create(destination)?
        }
        Err(e) => return Err(e),
    };
    const TARGET_UPDATES: u64 = 128;
    const MIN_CHUNK: usize = 4 * 1024 * 1024;
    let chunk_size = std::cmp::max(MIN_CHUNK, (file_size / TARGET_UPDATES) as usize);
    let mut total_copied = 0u64;
    loop {
        // NEW: Check for interrupt
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
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation aborted by user",
            ));
        }

        let to_copy = std::cmp::min(chunk_size, (file_size - total_copied) as usize);
        if to_copy == 0 {
            break;
        }
        match copy_file_range(&src_file, None, &dest_file, None, to_copy) {
            Ok(0) => break,
            Ok(copied) => {
                total_copied += copied as u64;
                if let Some(pb) = overall_pb {
                    pb.inc(copied as u64);
                }
            }
            Err(_) => {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
