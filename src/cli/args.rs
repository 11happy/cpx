use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct CLIArgs {
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,
    #[arg(required = true)]
    pub destination: PathBuf,

    #[arg(short, long, help = "Progress bar style: default, minimal, detailed")]
    pub style: Option<String>,

    #[arg(short, long, help = "Copy directories recursively")]
    pub recursive: bool,

    #[arg(
        short = 'j',
        default_value_t = 4,
        help = "Number of concurrent copy operations for multiple files"
    )]
    pub concurrency: usize,

    #[arg(
        short = 'c',
        long = "continue",
        help = "Continue copying by skipping files that are already complete"
    )]
    pub continue_copy: bool,

    #[arg(
        short = 'f',
        long,
        help = "if an existing destination file cannot be opened, remove it and try again"
    )]
    pub force: bool,

    #[arg(short = 'i', long, help = "prompt before overwrite")]
    pub interactive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CopyOptions {
    pub recursive: bool,
    pub concurrency: usize,
    pub resume: bool,
    pub force: bool,
    pub interactive: bool,
}

impl From<&CLIArgs> for CopyOptions {
    fn from(cli: &CLIArgs) -> Self {
        Self {
            recursive: cli.recursive,
            concurrency: cli.concurrency,
            resume: cli.continue_copy,
            force: cli.force,
            interactive: cli.interactive,
        }
    }
}
