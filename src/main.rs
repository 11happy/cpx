use clap::Parser;
use cpx::cli::args::{CLIArgs, CopyOptions};
use cpx::core::copy::{copy, multiple_copy};
use cpx::utility::progress_bar::ProgressBarStyle;

#[tokio::main]
async fn main() {
    let args = CLIArgs::parse();
    let style = match args.style.as_deref() {
        Some("minimal") => ProgressBarStyle::Minimal,
        Some("detailed") => ProgressBarStyle::Detailed,
        _ => ProgressBarStyle::Default,
    };
    let options = CopyOptions::from(&args);
    let result = if args.sources.len() == 1 {
        copy(&args.sources[0], &args.destination, style, &options).await
    } else {
        multiple_copy(args.sources, args.destination, style, &options).await
    };
    match result {
        Ok(_) => (),
        Err(e) => eprintln!("Error copying file: {}", e),
    }
}
