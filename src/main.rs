use cpx::cli::args::CLIArgs;
use cpx::core::copy::{copy, multiple_copy};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use std::process;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    // Use the custom parser instead of CLIArgs::parse()
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

    let mut signals = Signals::new([SIGINT, SIGTERM]).expect("Failed to setup signal handler");

    std::thread::spawn({
        let abort = abort.clone();
        move || {
            for sig in signals.forever() {
                match sig {
                    SIGINT | SIGTERM => {
                        eprintln!("\nInterrupt received — stopping gracefully...");
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
            // Success - normal exit
        }
        Err(e) => {
            // Check if interrupted
            if abort.load(Ordering::Relaxed) {
                eprintln!("\nOperation interrupted");
                eprintln!("Resume with: cpx --resume [original command]");
                eprintln!("Completed files will be skipped automatically)");
                process::exit(130); // Standard exit code for SIGINT
            } else {
                eprintln!("Error copying file: {}", e);
                process::exit(1);
            }
        }
    }
}
