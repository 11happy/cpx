use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CpxError {
    Io(io::Error),
    Config(ConfigError),
    Copy(CopyError),
    Exclude(ExcludeError),
    Preserve(PreserveError),
    Validation(String),
    OperationCancelled,
    InvalidPath(PathBuf),
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
    InvalidValue(String),
}

#[derive(Debug)]
pub enum CopyError {
    Io(io::Error),
    FileExists(PathBuf),
    PermissionDenied(PathBuf),
    InvalidSource(PathBuf),
    InvalidDestination(PathBuf),
    CopyFailed {
        source: PathBuf,
        destination: PathBuf,
        reason: String,
    },
    ReflinkFailed {
        source: PathBuf,
        destination: PathBuf,
    },
    HardlinkFailed {
        source: PathBuf,
        destination: PathBuf,
    },
    SymlinkFailed {
        source: PathBuf,
        destination: PathBuf,
    },
    PreserveFailed(PreserveError),
    /// The user answered "no" to an -i overwrite prompt; like GNU cp the
    /// copy continues but the exit status becomes 1.
    OverwriteDeclined(PathBuf),
}

#[derive(Debug)]
pub enum ExcludeError {
    InvalidPattern(String),
    PatternCompilation(globset::Error),
}

#[derive(Debug)]
pub enum PreserveError {
    Io(io::Error),
    UnsupportedAttribute(String),
    FailedToPreserve { path: PathBuf, attribute: String },
}

impl fmt::Display for CpxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpxError::Io(e) => write!(f, "IO error: {}", e),
            CpxError::Config(e) => write!(f, "Configuration error: {}", e),
            CpxError::Copy(e) => write!(f, "Copy error: {}", e),
            CpxError::Exclude(e) => write!(f, "Exclude pattern error: {}", e),
            CpxError::Preserve(e) => write!(f, "Preserve attribute error: {}", e),
            CpxError::Validation(msg) => write!(f, "Validation error: {}", msg),
            CpxError::OperationCancelled => write!(f, "Operation cancelled"),
            CpxError::InvalidPath(path) => write!(f, "Invalid path: {}", path.display()),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "Parse error: {}", e),
            ConfigError::InvalidValue(msg) => write!(f, "Invalid config value: {}", msg),
        }
    }
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopyError::Io(e) => write!(f, "IO error: {}", e),
            CopyError::OverwriteDeclined(path) => {
                write!(f, "not overwriting '{}'", path.display())
            }
            CopyError::FileExists(path) => write!(f, "File already exists: {}", path.display()),
            CopyError::PermissionDenied(path) => write!(f, "Permission denied: {}", path.display()),
            CopyError::InvalidSource(path) => write!(f, "Invalid source path: {}", path.display()),
            CopyError::InvalidDestination(path) => {
                write!(f, "Invalid destination path: {}", path.display())
            }
            CopyError::CopyFailed {
                source,
                destination,
                reason,
            } => {
                write!(
                    f,
                    "Failed to copy '{}' to '{}': {}",
                    source.display(),
                    destination.display(),
                    reason
                )
            }
            CopyError::ReflinkFailed {
                source,
                destination,
            } => {
                write!(
                    f,
                    "Reflink failed from '{}' to '{}'",
                    source.display(),
                    destination.display()
                )
            }
            CopyError::HardlinkFailed {
                source,
                destination,
            } => {
                write!(
                    f,
                    "Hardlink failed from '{}' to '{}'",
                    source.display(),
                    destination.display()
                )
            }
            CopyError::SymlinkFailed {
                source,
                destination,
            } => {
                write!(
                    f,
                    "Symlink failed from '{}' to '{}'",
                    source.display(),
                    destination.display()
                )
            }
            CopyError::PreserveFailed(e) => write!(f, "Preserve failed: {}", e),
        }
    }
}

impl fmt::Display for ExcludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExcludeError::InvalidPattern(pattern) => {
                write!(f, "Invalid exclude pattern: {}", pattern)
            }
            ExcludeError::PatternCompilation(e) => write!(f, "Pattern compilation error: {}", e),
        }
    }
}

impl fmt::Display for PreserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreserveError::Io(e) => write!(f, "IO error: {}", e),
            PreserveError::UnsupportedAttribute(attr) => {
                write!(f, "Unsupported preserve attribute: {}", attr)
            }
            PreserveError::FailedToPreserve { path, attribute } => {
                write!(
                    f,
                    "Failed to preserve '{}' for '{}'",
                    attribute,
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CpxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CpxError::Io(e) => Some(e),
            CpxError::Config(e) => Some(e),
            CpxError::Copy(e) => Some(e),
            CpxError::Exclude(e) => Some(e),
            CpxError::Preserve(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for CopyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CopyError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for ExcludeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExcludeError::PatternCompilation(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for PreserveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PreserveError::Io(e) => Some(e),
            _ => None,
        }
    }
}

// Conversion traits
impl From<io::Error> for CpxError {
    fn from(e: io::Error) -> Self {
        CpxError::Io(e)
    }
}

impl From<ConfigError> for CpxError {
    fn from(e: ConfigError) -> Self {
        CpxError::Config(e)
    }
}

impl From<CopyError> for CpxError {
    fn from(e: CopyError) -> Self {
        CpxError::Copy(e)
    }
}

impl From<ExcludeError> for CpxError {
    fn from(e: ExcludeError) -> Self {
        CpxError::Exclude(e)
    }
}

impl From<PreserveError> for CpxError {
    fn from(e: PreserveError) -> Self {
        CpxError::Preserve(e)
    }
}

impl From<PreserveError> for CopyError {
    fn from(e: PreserveError) -> Self {
        CopyError::PreserveFailed(e)
    }
}

impl CopyError {
    pub fn kind(&self) -> io::ErrorKind {
        match self {
            CopyError::Io(e) => e.kind(),
            CopyError::FileExists(_) => io::ErrorKind::AlreadyExists,
            CopyError::PermissionDenied(_) => io::ErrorKind::PermissionDenied,
            CopyError::InvalidSource(_) => io::ErrorKind::NotFound,
            CopyError::InvalidDestination(_) => io::ErrorKind::NotFound,
            CopyError::CopyFailed { .. } => io::ErrorKind::Other,
            CopyError::ReflinkFailed { .. } => io::ErrorKind::Unsupported,
            CopyError::HardlinkFailed { .. } => io::ErrorKind::Other,
            CopyError::SymlinkFailed { .. } => io::ErrorKind::Other,
            CopyError::PreserveFailed(_) => io::ErrorKind::Other,
            CopyError::OverwriteDeclined(_) => io::ErrorKind::Other,
        }
    }
}

impl From<io::Error> for CopyError {
    fn from(e: io::Error) -> Self {
        CopyError::Io(e)
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Parse(e)
    }
}

impl From<globset::Error> for ExcludeError {
    fn from(e: globset::Error) -> Self {
        ExcludeError::PatternCompilation(e)
    }
}

impl From<io::Error> for PreserveError {
    fn from(e: io::Error) -> Self {
        PreserveError::Io(e)
    }
}

// Result type alias
pub type CpxResult<T> = Result<T, CpxError>;
pub type ConfigResult<T> = Result<T, ConfigError>;
pub type CopyResult<T> = Result<T, CopyError>;
pub type ExcludeResult<T> = Result<T, ExcludeError>;
pub type PreserveResult<T> = Result<T, PreserveError>;
