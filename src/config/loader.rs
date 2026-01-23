use super::schema::Config;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    InvalidValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "Parse error: {}", e),
            ConfigError::InvalidValue(msg) => write!(f, "Invalid config value: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn find_config_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let project_config = PathBuf::from("./cpxconfig.toml");
    if project_config.exists() {
        paths.push(project_config);
    }
    if let Some(config_dir) = dirs::config_dir() {
        let user_config = config_dir.join("cpx").join("cpxconfig.toml");
        if user_config.exists() {
            paths.push(user_config);
        }
    }
    #[cfg(unix)]
    {
        let system_config = PathBuf::from("/etc/cpx/cpxconfig.toml");
        if system_config.exists() {
            paths.push(system_config);
        }
    }
    paths
}

pub fn load_config_file(path: &Path) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(path).map_err(ConfigError::Io)?;
    let config: Config = toml::from_str(&contents).map_err(ConfigError::Parse)?;
    Ok(config)
}

/// Load and merge all config files (reverse priority: system < user < project)
pub fn load_config() -> Config {
    let project = PathBuf::from("./cpxconfig.toml");
    if project.exists()
        && let Ok(config) = load_config_file(&project)
    {
        return config;
    }

    if let Some(config_dir) = dirs::config_dir() {
        let user = config_dir.join("cpx").join("cpxconfig.toml");
        if user.exists()
            && let Ok(config) = load_config_file(&user)
        {
            return config;
        }
    }

    #[cfg(unix)]
    {
        let system = PathBuf::from("/etc/cpx/cpxconfig.toml");
        if system.exists()
            && let Ok(config) = load_config_file(&system)
        {
            return config;
        }
    }

    Config::default()
}
