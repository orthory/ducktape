use std::path::Path;
use serde::{Deserialize, Serialize};

pub(crate) mod cadence;
pub(crate) mod journal;

/// Configuration for the document subsystem. Loaded from a TOML file at
/// startup; used to tune runtime parameters that previously lived as
/// compile-time constants (journal buffer size, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub journal: crate::config::journal::Config,
    pub cadence: crate::config::cadence::Config,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("config: io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("config: parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

impl Config {
    /// Parse a TOML string into a `DocumentConfig`. Use this when the config
    /// content has already been read from somewhere.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(s)?)
    }

    /// Read a TOML file from disk and parse it. Convenience wrapper around
    /// [`Self::from_toml_str`].
    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&content)
    }
}
