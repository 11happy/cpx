use crate::utility::preserve::PreserveAttr;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct CLIArgs {
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,

    #[arg(required = true)]
    pub destination: PathBuf,

    #[arg(
        short = 't',
        long = "target-directory",
        value_name = "DIRECTORY",
        help = "copy all SOURCE arguments into DIRECTORY"
    )]
    pub target_directory: Option<PathBuf>,

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

    #[arg(long, help = "use full source file name under DIRECTORY")]
    pub parents: bool,

    #[arg(
        short = 'p',
        long = "preserve",
        value_name = "ATTR_LIST",
        default_missing_value = "",
        help = "preserve the specified attributes"
    )]
    pub preserve: Option<String>,

    #[arg(
        long = "attributes-only",
        help = "don't copy the file data, just the attributes"
    )]
    pub attributes_only: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CopyOptions {
    pub recursive: bool,
    pub concurrency: usize,
    pub resume: bool,
    pub force: bool,
    pub interactive: bool,
    pub parents: bool,
    pub preserve: PreserveAttr,
    pub attributes_only: bool,
}

impl From<&CLIArgs> for CopyOptions {
    fn from(cli: &CLIArgs) -> Self {
        Self {
            recursive: cli.recursive,
            concurrency: cli.concurrency,
            resume: cli.continue_copy,
            force: cli.force,
            interactive: cli.interactive,
            parents: cli.parents,
            preserve: match &cli.preserve {
                None => PreserveAttr::none(),
                Some(s) => {
                    PreserveAttr::from_string(s).expect("unable to parse preserve attribute")
                }
            },
            attributes_only: cli.attributes_only,
        }
    }
}

impl CLIArgs {
    pub fn validate(mut self) -> Result<(Vec<PathBuf>, PathBuf, CopyOptions), String> {
        let mut options = CopyOptions::from(&self);
        if options.attributes_only {
            options.preserve = PreserveAttr::all();
        }
        let (sources, destination) = if let Some(target) = self.target_directory {
            self.sources.push(self.destination);
            (self.sources, target)
        } else {
            (self.sources, self.destination)
        };

        Ok((sources, destination, options))
    }
}
