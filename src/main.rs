use cpx::cli::args::CLIArgs;
use cpx::core::copy::{copy, multiple_copy};
use cpx::error::{CopyError, CpxError};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use std::process;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    // custom parser
    let args = CLIArgs::parse();

    let (sources, destination, mut options) = match args.validate() {
        Ok(validated) => validated,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    let abort = Arc::new(AtomicBool::new(false));
    options.abort = abort.clone();

    let mut signals = Signals::new([SIGINT, SIGTERM])
        .map_err(CpxError::Io)
        .unwrap_or_else(|e| {
            eprintln!("Failed to setup signal handler: {}", e);
            process::exit(1);
        });

    std::thread::spawn({
        let abort = abort.clone();
        move || {
            for sig in signals.forever() {
                match sig {
                    SIGINT | SIGTERM => {
                        abort.store(true, Ordering::Relaxed);
                    }
                    _ => unreachable!(),
                }
            }
        }
    });

    let result = if sources.len() == 1 {
        copy(&sources[0], &destination, &options)
    } else {
        multiple_copy(sources, destination, &options)
    };

    match result {
        Ok(_) => {
            // normal
        }
        Err(e) => {
            // interrupt check
            if abort.load(Ordering::Relaxed) {
                eprintln!("\nOperation interrupted");
                eprintln!("Resume with: cpx --resume [original command]");
                eprintln!("Completed files will be skipped automatically");
                process::exit(130); // SIGINT
            } else if let CopyError::OverwriteDeclined(_) = e {
                // GNU cp: declining an -i prompt is silent but exits 1.
                process::exit(1);
            } else {
                eprintln!("Error copying file: {}", e);
                process::exit(1);
            }
        }
    }
}
