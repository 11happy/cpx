use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ExcludeConfig {
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CopyConfig {
    pub parallel: usize,
    pub recursive: bool,
    pub delete_source: bool,
    pub parents: bool,
    pub force: bool,
    pub interactive: bool,
    pub resume: bool,
    pub attributes_only: bool,
    pub remove_destination: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreserveConfig {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SymlinkConfig {
    pub mode: String,   // "auto", "absolute", "relative"
    pub follow: String, // "never", "always", "command-line"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub mode: String, // "none", "simple", "numbered", "existing"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflinkConfig {
    pub mode: String, // "auto", "always", "never"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgressConfig {
    pub style: String, // "default", "detailed"
    pub bar: ProgressBarConfig,
    pub color: ProgressColorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgressBarConfig {
    pub filled: String,
    pub empty: String,
    pub head: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgressColorConfig {
    pub bar: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub exclude: ExcludeConfig,
    pub copy: CopyConfig,
    pub preserve: PreserveConfig,
    pub symlink: SymlinkConfig,
    pub backup: BackupConfig,
    pub reflink: ReflinkConfig,
    pub progress: ProgressConfig,
}

impl Default for CopyConfig {
    fn default() -> Self {
        Self {
            delete_source: false,
            parallel: 4,
            recursive: false,
            parents: false,
            force: false,
            interactive: false,
            resume: false,
            attributes_only: false,
            remove_destination: false,
        }
    }
}

impl Default for PreserveConfig {
    fn default() -> Self {
        Self {
            mode: "default".to_string(),
        }
    }
}

impl Default for SymlinkConfig {
    fn default() -> Self {
        Self {
            mode: "".to_string(),
            follow: "".to_string(),
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            mode: "none".to_string(),
        }
    }
}

impl Default for ReflinkConfig {
    fn default() -> Self {
        Self {
            mode: "".to_string(),
        }
    }
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            style: "default".to_string(),
            bar: ProgressBarConfig::default(),
            color: ProgressColorConfig::default(),
        }
    }
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            filled: "█".to_string(),
            empty: "░".to_string(),
            head: "░".to_string(),
        }
    }
}

impl Default for ProgressColorConfig {
    fn default() -> Self {
        Self {
            bar: "white".to_string(),
            message: "white".to_string(),
        }
    }
}

impl Config {
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}
