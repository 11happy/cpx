use crate::config::config_command::ConfigCommand;
use crate::config::loader::{load_config, load_config_file};
use crate::config::schema::Config;
use crate::error::{CpxError, CpxResult};
use crate::utility::helper::parse_progress_bar;
use crate::utility::progress_bar::ProgressOptions;
use crate::utility::{
    exclude::{ExcludePattern, ExcludeRules, build_exclude_rules, parse_exclude_pattern_list},
    helper::{parse_backup_mode, parse_follow_symlink, parse_reflink_mode, parse_symlink_mode},
    preserve::PreserveAttr,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SymlinkMode {
    Auto,
    Absolute,
    Relative,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum ReflinkMode {
    Always,
    Auto,
    Clone,
    Copy,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum BackupMode {
    None,
    Numbered,
    Existing,
    Simple,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FollowSymlink {
    NoDereference,
    Dereference,
    CommandLineSymlink,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Default (Implicit)
    Copy(CopyArgs),

    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Parser, Debug)]
#[command(name = "cpx",version = env!("CARGO_PKG_VERSION"))]
pub struct CLIArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args, Debug, Clone)]
pub struct CopyArgs {
    // Input/Output Options
    #[arg(help = "Source file(s) or directory(ies)", required = true)]
    pub sources: Vec<PathBuf>,

    #[arg(help = "Destination file or directory", required = true)]
    pub destination: PathBuf,

    #[arg(
        short = 't',
        long = "target-directory",
        value_name = "DIRECTORY",
        help = "copy all SOURCE arguments into DIRECTORY"
    )]
    pub target_directory: Option<PathBuf>,

    #[arg(
        short = 'e',
        long = "exclude",
        value_name = "PATTERN",
        help = "Exclude files matching pattern (can be specified multiple times, supports comma-separated values)"
    )]
    pub exclude: Vec<String>,

    // Copy Behavior Options
    #[arg(short, long, help = "Copy directories recursively")]
    pub recursive: bool,

    #[arg(short, long, help = "Deletes the original file(s) after copying")]
    pub delete_source: bool,

    #[arg(
        short = 'j',
        default_value_t = 4,
        help = "Number of parallel copy operations for multiple files"
    )]
    pub parallel: usize,

    #[arg(long = "resume", help = "resume interrupted transfers")]
    pub resume: bool,

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
        long = "attributes-only",
        help = "don't copy the file data, just the attributes"
    )]
    pub attributes_only: bool,

    #[arg(
        long = "remove-destination",
        help = "remove each existing destination file before attempting to open it"
    )]
    pub remove_destination: bool,

    // Link and Symlink Options
    #[arg(
        short = 's',
        long = "symbolic-link",
        value_name = "MODE",
        default_missing_value = "auto",
        num_args = 0..=1,
        help = "make symbolic links instead of copying (auto, absolute, or relative)"
    )]
    pub symbolic_link: Option<SymlinkMode>,

    #[arg(
        short = 'l',
        long = "link",
        help = "hard link files instead of copying"
    )]
    pub hard_link: bool,

    #[arg(
        short = 'P',
        long = "no-dereference",
        help = "never follow symbolic links in SOURCE"
    )]
    pub no_dereference: bool,

    #[arg(
        short = 'L',
        long = "dereference",
        help = "always follow symbolic links in SOURCE"
    )]
    pub dereference: bool,

    #[arg(
        short = 'H',
        long = "dereference-command-line",
        help = "follow symbolic links only on command line"
    )]
    pub dereference_command_line: bool,

    // Preservation Options
    #[arg(
        short = 'p',
        long = "preserve",
        value_name = "ATTR_LIST",
        default_missing_value = "",
        help = "preserve the specified attributes"
    )]
    pub preserve: Option<String>,

    // Backup and Reflink Options
    #[arg(
        short = 'b',
        long = "backup",
        value_name = "CONTROL",
        default_missing_value = "existing",
        num_args = 0..=1,
        help = "make a backup of each existing destination file (none, numbered, existing, simple)"
    )]
    pub backup: Option<BackupMode>,

    #[arg(
        long = "reflink",
        value_name = "WHEN",
        default_missing_value = "auto",
        num_args = 0..=1,
        help = "control clone/CoW copies (auto, always, never)"
    )]
    pub reflink: Option<ReflinkMode>,

    // Config Options (Placed last as meta)
    #[arg(long, value_name = "PATH", help = "Use custom config file")]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Ignore all config files")]
    pub no_config: bool,
}

#[derive(Debug, Clone)]
pub struct CopyOptions {
    pub recursive: bool,
    pub delete_source: bool,
    pub parallel: usize,
    pub resume: bool,
    pub force: bool,
    pub interactive: bool,
    pub parents: bool,
    pub preserve: PreserveAttr,
    pub attributes_only: bool,
    pub remove_destination: bool,
    pub symbolic_link: Option<SymlinkMode>,
    pub hard_link: bool,
    pub follow_symlink: FollowSymlink,
    pub progress_bar: ProgressOptions,
    pub backup: Option<BackupMode>,
    pub reflink: Option<ReflinkMode>,
    pub exclude_rules: Option<ExcludeRules>,
    pub abort: Arc<AtomicBool>,
}

impl CopyOptions {
    pub fn none() -> Self {
        Self {
            recursive: false,
            delete_source: false,
            parallel: 4,
            resume: false,
            force: false,
            interactive: false,
            parents: false,
            preserve: PreserveAttr::none(),
            attributes_only: false,
            remove_destination: false,
            symbolic_link: None,
            hard_link: false,
            follow_symlink: FollowSymlink::NoDereference,
            progress_bar: ProgressOptions::default(),
            backup: None,
            reflink: None,
            exclude_rules: None,
            abort: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn from_config(config: &Config) -> Self {
        Self {
            recursive: config.copy.recursive,
            delete_source: config.copy.delete_source,
            parallel: config.copy.parallel,
            resume: config.copy.resume,
            force: config.copy.force,
            interactive: config.copy.interactive,
            parents: config.copy.parents,
            preserve: PreserveAttr::from_string(&config.preserve.mode)
                .unwrap_or_else(|_| PreserveAttr::default()),
            attributes_only: config.copy.attributes_only,
            remove_destination: config.copy.remove_destination,
            symbolic_link: parse_symlink_mode(&config.symlink.mode),
            hard_link: false,
            follow_symlink: parse_follow_symlink(&config.symlink.follow),
            progress_bar: parse_progress_bar(config),
            backup: parse_backup_mode(&config.backup.mode),
            reflink: parse_reflink_mode(&config.reflink.mode),
            exclude_rules: None,
            abort: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl From<&CopyArgs> for CopyOptions {
    fn from(cli: &CopyArgs) -> Self {
        Self {
            recursive: cli.recursive,
            delete_source: cli.delete_source,
            parallel: cli.parallel,
            resume: cli.resume,
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
            remove_destination: cli.remove_destination,
            symbolic_link: cli.symbolic_link,
            hard_link: cli.hard_link,
            follow_symlink: FollowSymlink::NoDereference,
            progress_bar: ProgressOptions::default(),
            backup: cli.backup,
            reflink: cli.reflink,
            exclude_rules: None,
            abort: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl CLIArgs {
    /// Parse arguments with implicit copy command support
    pub fn parse() -> Self {
        let mut args: Vec<String> = std::env::args().collect();

        if args.len() > 1 {
            let first_arg = &args[1];
            let is_subcommand = matches!(
                first_arg.as_str(),
                "config" | "copy" | "-h" | "--help" | "-V" | "--version"
            );
            if !is_subcommand {
                args.insert(1, "copy".to_string());
                return <Self as clap::Parser>::parse_from(args);
            }
        }
        <Self as clap::Parser>::parse()
    }

    pub fn validate(self) -> CpxResult<(Vec<PathBuf>, PathBuf, CopyOptions)> {
        // Handle config command
        if let Commands::Config { command } = &self.command {
            command.execute().map_err(|e| {
                CpxError::Validation(format!("Failed to execute config command: {}", e))
            })?;
            std::process::exit(0);
        }

        // Get copy args from the Copy subcommand
        let copy_args = match self.command {
            Commands::Copy(args) => args,
            _ => unreachable!(),
        };

        let config = load_config_if_needed(&copy_args).map_err(CpxError::Config)?;

        // Start with config or defaults
        let mut options = if let Some(ref cfg) = config {
            CopyOptions::from_config(cfg)
        } else {
            CopyOptions::none()
        };

        // CLI args override config
        apply_cli_overrides(&mut options, &copy_args).map_err(CpxError::Validation)?;

        // Build exclude rules
        let all_patterns =
            build_all_exclude_patterns(&copy_args, config.as_ref()).map_err(CpxError::Exclude)?;
        options.exclude_rules = build_exclude_rules(all_patterns).map_err(CpxError::Exclude)?;

        // Validate conflicts
        validate_conflicts(&options).map_err(CpxError::Validation)?;

        // Handle attributes_only special case
        if options.attributes_only {
            options.preserve = PreserveAttr::all();
        }

        let (sources, destination) = if let Some(target) = copy_args.target_directory {
            let mut sources = copy_args.sources;
            sources.push(copy_args.destination);
            (sources, target)
        } else {
            (copy_args.sources, copy_args.destination)
        };

        Ok((sources, destination, options))
    }
}

fn load_config_if_needed(copy_args: &CopyArgs) -> crate::error::ConfigResult<Option<Config>> {
    if copy_args.no_config {
        return Ok(None);
    }

    if let Some(custom_path) = &copy_args.config {
        return Ok(Some(load_config_file(custom_path)?));
    }

    Ok(Some(load_config()))
}

fn apply_cli_overrides(options: &mut CopyOptions, copy_args: &CopyArgs) -> Result<(), String> {
    // Boolean flags - when present, they override
    if copy_args.recursive {
        options.recursive = true;
    }
    if copy_args.delete_source {
        options.delete_source = true;
    }
    if copy_args.force {
        options.force = true;
    }
    if copy_args.interactive {
        options.interactive = true;
    }
    if copy_args.resume {
        options.resume = true;
    }
    if copy_args.parents {
        options.parents = true;
    }
    if copy_args.attributes_only {
        options.attributes_only = true;
    }
    if copy_args.remove_destination {
        options.remove_destination = true;
    }
    if copy_args.hard_link {
        options.hard_link = true;
    }

    // Optional fields - when Some, they override
    if copy_args.symbolic_link.is_some() {
        options.symbolic_link = copy_args.symbolic_link;
    }
    if copy_args.backup.is_some() {
        options.backup = copy_args.backup;
    }
    if copy_args.reflink.is_some() {
        options.reflink = copy_args.reflink;
    }
    if let Some(preserve_str) = &copy_args.preserve {
        options.preserve = PreserveAttr::from_string(preserve_str)
            .map_err(|e| format!("unable to parse preserve attribute: {}", e))?;
    }

    options.parallel = copy_args.parallel;

    options.follow_symlink = copy_args.follow_symlink_mode()?;

    Ok(())
}

fn build_all_exclude_patterns(
    copy_args: &CopyArgs,
    config: Option<&Config>,
) -> crate::error::ExcludeResult<Vec<ExcludePattern>> {
    let mut all_patterns = Vec::new();

    if let Some(cfg) = config {
        for pattern_str in &cfg.exclude.patterns {
            all_patterns.extend(parse_exclude_pattern_list(pattern_str)?);
        }
    }

    all_patterns.extend(copy_args.parse_exclude_patterns()?);
    Ok(all_patterns)
}

fn validate_conflicts(options: &CopyOptions) -> Result<(), String> {
    if options.reflink.is_some() {
        if options.hard_link {
            return Err("--reflink and --link cannot be used together".to_string());
        }
        if options.symbolic_link.is_some() {
            return Err("--reflink and --symbolic-link cannot be used together".to_string());
        }
    }

    if options.symbolic_link.is_some() {
        if options.hard_link {
            return Err("--symbolic-link and --link cannot be used together".to_string());
        }
        if options.resume {
            return Err("--symbolic-link and --continue cannot be used together".to_string());
        }
        if options.attributes_only {
            return Err(
                "--symbolic-link and --attributes-only cannot be used together".to_string(),
            );
        }
    }

    if options.hard_link {
        if options.resume {
            return Err("--link and --continue cannot be used together".to_string());
        }
        if options.attributes_only {
            return Err("--link and --attributes-only cannot be used together".to_string());
        }
    }

    Ok(())
}

impl CopyArgs {
    pub fn follow_symlink_mode(&self) -> Result<FollowSymlink, String> {
        match (
            self.no_dereference,
            self.dereference,
            self.dereference_command_line,
        ) {
            (true, false, false) => Ok(FollowSymlink::NoDereference),
            (false, true, false) => Ok(FollowSymlink::Dereference),
            (false, false, true) => Ok(FollowSymlink::CommandLineSymlink),
            (false, false, false) => Ok(FollowSymlink::NoDereference),
            _ => Err("only one of -P, -L, or -H may be specified".to_string()),
        }
    }

    pub fn parse_exclude_patterns(&self) -> crate::error::ExcludeResult<Vec<ExcludePattern>> {
        let mut patterns = Vec::new();

        for pattern_str in &self.exclude {
            patterns.extend(parse_exclude_pattern_list(pattern_str)?);
        }

        Ok(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_symlink_and_hardlink_conflict() {
        let args = CLIArgs {
            command: Commands::Copy(CopyArgs {
                sources: vec![PathBuf::from("source.txt")],
                destination: PathBuf::from("dest.txt"),
                target_directory: None,
                recursive: false,
                delete_source: false,
                parallel: 4,
                resume: false,
                force: false,
                interactive: false,
                parents: false,
                preserve: None,
                attributes_only: false,
                remove_destination: false,
                symbolic_link: Some(SymlinkMode::Auto),
                hard_link: true,
                dereference: true,
                no_dereference: false,
                dereference_command_line: false,
                backup: None,
                reflink: None,
                exclude: Vec::new(),
                no_config: false,
                config: None,
            }),
        };

        let result = args.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("symbolic-link"));
    }

    #[test]
    fn test_validate_symlink_and_resume_conflict() {
        let args = CLIArgs {
            command: Commands::Copy(CopyArgs {
                sources: vec![PathBuf::from("source.txt")],
                destination: PathBuf::from("dest.txt"),
                target_directory: None,
                recursive: false,
                delete_source: false,
                parallel: 4,
                resume: true,
                force: false,
                interactive: false,
                parents: false,
                preserve: None,
                attributes_only: false,
                remove_destination: false,
                symbolic_link: Some(SymlinkMode::Auto),
                hard_link: false,
                dereference: true,
                no_dereference: false,
                dereference_command_line: false,
                backup: None,
                reflink: None,
                exclude: Vec::new(),
                no_config: false,
                config: None,
            }),
        };

        let result = args.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("continue"));
    }

    #[test]
    fn test_validate_hardlink_and_resume_conflict() {
        let args = CLIArgs {
            command: Commands::Copy(CopyArgs {
                sources: vec![PathBuf::from("source.txt")],
                destination: PathBuf::from("dest.txt"),
                target_directory: None,
                recursive: false,
                delete_source: false,
                parallel: 4,
                resume: true,
                force: false,
                interactive: false,
                parents: false,
                preserve: None,
                attributes_only: false,
                remove_destination: false,
                symbolic_link: None,
                hard_link: true,
                dereference: true,
                no_dereference: false,
                dereference_command_line: false,
                backup: None,
                reflink: None,
                exclude: Vec::new(),
                no_config: false,
                config: None,
            }),
        };

        let result = args.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("link"));
    }

    #[test]
    fn test_validate_success() {
        let args = CLIArgs {
            command: Commands::Copy(CopyArgs {
                sources: vec![PathBuf::from("source.txt")],
                destination: PathBuf::from("dest.txt"),
                target_directory: None,
                recursive: false,
                delete_source: false,
                parallel: 4,
                resume: false,
                force: false,
                interactive: false,
                parents: false,
                preserve: None,
                attributes_only: false,
                remove_destination: false,
                symbolic_link: None,
                hard_link: false,
                dereference: true,
                no_dereference: false,
                dereference_command_line: false,
                backup: None,
                reflink: None,
                exclude: Vec::new(),
                no_config: false,
                config: None,
            }),
        };

        let result = args.validate();
        assert!(result.is_ok());
    }
}
