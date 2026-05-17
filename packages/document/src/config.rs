use std::path::Path;

use serde::{Deserialize, Serialize};

/// Configuration for the document subsystem. Loaded from a TOML file at
/// startup; used to tune runtime parameters that previously lived as
/// compile-time constants (journal buffer size, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentConfig {
    pub journal: JournalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalConfig {
    /// High-water mark — number of ops per Journal before it rotates into
    /// `to_be_flushed` and a fresh journal is opened.
    pub hwm: usize,
}

impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            journal: JournalConfig::default(),
        }
    }
}

impl Default for JournalConfig {
    fn default() -> Self {
        Self { hwm: 8192 }
    }
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

impl DocumentConfig {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sensible_hwm() {
        let cfg = DocumentConfig::default();
        assert!(cfg.journal.hwm >= 64);
    }

    #[test]
    fn parses_well_formed_toml() {
        let toml = r#"
[journal]
hwm = 1024
"#;
        let cfg = DocumentConfig::from_toml_str(toml).expect("parse ok");
        assert_eq!(cfg.journal.hwm, 1024);
    }

    #[test]
    fn rejects_malformed_toml() {
        let toml = "not = valid = toml";
        assert!(DocumentConfig::from_toml_str(toml).is_err());
    }

    #[test]
    fn rejects_missing_required_field() {
        let toml = "[journal]\n";
        assert!(DocumentConfig::from_toml_str(toml).is_err());
    }
}
