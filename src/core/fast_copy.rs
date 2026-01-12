use indicatif::ProgressBar;
use nix::fcntl::copy_file_range;
use std::io;
use std::path::Path;

pub fn fast_copy(
    source: &Path,
    destination: &Path,
    file_size: u64,
    overall_pb: Option<&ProgressBar>,
) -> io::Result<bool> {
    let src_file = std::fs::File::open(source)?;
    let dest_file = std::fs::File::create(destination)?;
    const TARGET_UPDATES: u64 = 128;
    const MIN_CHUNK: usize = 4 * 1024 * 1024;
    let chunk_size = std::cmp::max(MIN_CHUNK, (file_size / TARGET_UPDATES) as usize);
    let mut total_copied = 0u64;
    loop {
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
