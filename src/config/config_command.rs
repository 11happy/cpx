use super::loader::{find_config_files, load_config};
use super::schema::Config;
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::io::Write;

#[derive(Debug, Subcommand, Clone)]
pub enum ConfigCommand {
    /// Initialize a new config file with defaults
    Init {
        #[arg(short, long, help = "Overwrite existing config file")]
        force: bool,
    },
    /// Show current config
    Show,
    /// Show config file locations
    Path,
}

impl ConfigCommand {
    pub fn execute(&self) -> std::io::Result<()> {
        match self {
            ConfigCommand::Init { force } => init_config(*force),
            ConfigCommand::Show => show_config(),
            ConfigCommand::Path => show_paths(),
        }
    }
}

fn init_config(force: bool) -> std::io::Result<()> {
    // Get user config directory
    let config_dir = dirs::config_dir()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            )
        })?
        .join("cpx");

    let config_path = config_dir.join("cpxconfig.toml");

    // Check if config already exists
    if config_path.exists() && !force {
        eprintln!(
            "{} Config file already exists at: {}",
            "Error:".red().bold(),
            config_path.display()
        );
        eprintln!("Use --force to overwrite");
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Config file already exists",
        ));
    }

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)?;

    // Generate default config
    let default_config = Config::default();
    let toml_content = default_config
        .to_toml_string()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Add comments to make it more user-friendly
    let commented_content = add_comments_to_config(&toml_content);

    // Write config file
    let mut file = fs::File::create(&config_path)?;
    file.write_all(commented_content.as_bytes())?;

    println!(
        "Created config file at: {}",
        config_path.display().to_string().cyan()
    );
    Ok(())
}

fn show_config() -> std::io::Result<()> {
    let config_files = find_config_files();

    if config_files.is_empty() {
        println!("{} No config files found", "Info:".yellow().bold());
        println!("\nCreate one with: {}", "cpx config init".green());
        return Ok(());
    }

    // Load and merge configs
    let merged_config = load_config();

    // Display the effective configuration
    println!("{}", "Current Configuration:".bold().underline());
    println!();

    let toml_string = merged_config
        .to_toml_string()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Pretty print with syntax highlighting
    for line in toml_string.lines() {
        if line.starts_with('[') {
            println!("{}", line.bright_blue().bold());
        } else if line.contains('=') {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                print!("{}", parts[0].green());
                print!("{}", "=".white());
                println!("{}", parts[1].yellow());
            } else {
                println!("{}", line);
            }
        } else {
            println!("{}", line.dimmed());
        }
    }

    Ok(())
}

fn show_paths() -> std::io::Result<()> {
    use std::path::PathBuf;

    println!("{}", "Effective Config File".bold().underline());
    println!();

    let mut effective: Option<PathBuf> = None;

    // 1️⃣ Project config
    let project = PathBuf::from("./cpxconfig.toml");
    if project.exists() {
        effective = Some(project);
    }

    // 2️⃣ User config
    if effective.is_none()
        && let Some(config_dir) = dirs::config_dir()
    {
        let user = config_dir.join("cpx").join("cpxconfig.toml");
        if user.exists() {
            effective = Some(user);
        }
    }

    // 3️⃣ System config (Unix)
    #[cfg(unix)]
    if effective.is_none() {
        let system = PathBuf::from("/etc/cpx/config.toml");
        if system.exists() {
            effective = Some(system);
        }
    }

    match effective {
        Some(path) => {
            println!("{}", path.display().to_string().cyan());
        }
        None => {
            println!("{}", "No config file found — using defaults".dimmed());
        }
    }

    println!();
    println!("{}", "Priority Order:".bold());
    println!("  CLI flags > Project config > User config > System config > Defaults");

    Ok(())
}

fn add_comments_to_config(toml: &str) -> String {
    let header = r#"# cpx configuration file
# For more information, see: https://github.com/11happy/cpx/docs/configuration.md

"#;

    let mut result = String::from(header);

    for line in toml.lines() {
        // Add section comments
        match line {
            l if l.starts_with("[exclude]") => {
                result.push_str("# Exclude patterns (glob syntax supported)\n");
                result.push_str(
                    "# Example: patterns = [\"*.tmp\", \"*.log\", \"node_modules\", \".git\"]\n",
                );
            }
            l if l.starts_with("[copy]") => {
                result.push_str("\n# Copy operation settings\n");
            }
            l if l.starts_with("[preserve]") => {
                result.push_str("\n# Preserve file attributes\n");
                result.push_str("# mode values: \"none\", \"default\", \"all\", or \"mode,timestamps,ownership\"\n");
            }
            l if l.starts_with("[symlink]") => {
                result.push_str("\n# Symlink handling\n");
                result.push_str("# mode: \"auto\", \"absolute\", \"relative\"\n");
                result
                    .push_str("# follow: \"never\" (-P), \"always\" (-L), \"command-line\" (-H)\n");
            }
            l if l.starts_with("[backup]") => {
                result.push_str("\n# Backup settings\n");
                result.push_str(
                    "# mode: \"none\", \"simple\" (~), \"numbered\" (~1~, ~2~), \"existing\"\n",
                );
            }
            l if l.starts_with("[reflink]") => {
                result.push_str("\n# Copy-on-Write (reflink) settings\n");
                result.push_str("# mode: \"auto\", \"always\", \"never\"\n");
            }
            l if l.starts_with("[progress]") => {
                result.push_str("\n# Progress bar settings\n");
            }
            l if l.starts_with("[progress.bar]") => {
                result.push_str("# Progress bar characters\n");
            }
            l if l.starts_with("[progress.color]") => {
                result.push_str("# Supported progress bar colors: black, red, green, yellow, blue, magenta, cyan, white\n");
            }
            l if l.starts_with("[progress.behavior]") => {
                result.push_str("# Progress bar behavior\n");
            }
            _ => {}
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}
