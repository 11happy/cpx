use cpx::cli::args::CLIArgs;
use cpx::core::copy::{copy, multiple_copy};
use std::process;

fn main() {
    // Use the custom parser instead of CLIArgs::parse()
    let args = CLIArgs::parse();

    let (sources, destination, options) = match args.validate() {
        Ok(validated) => validated,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    let result = if sources.len() == 1 {
        copy(&sources[0], &destination, &options)
    } else {
        multiple_copy(sources, destination, &options)
    };

    if let Err(e) = result {
        eprintln!("Error copying file: {}", e);
        process::exit(1);
    }
}
