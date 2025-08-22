//! Error types for SwissArmyHammer configuration system

use std::path::PathBuf;
use thiserror::Error;

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: PathBuf },

    /// Configuration file could not be read
    #[error("Failed to read configuration file {path}: {source}")]
    FileReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Configuration parsing failed
    #[error("Failed to parse configuration{}: {source}", path.as_ref().map(|p| format!(" from {}", p.display())).unwrap_or_default())]
    ParseError {
        path: Option<PathBuf>,
        #[source]
        source: figment::Error,
    },

    /// Configuration validation failed
    #[error("Configuration validation failed: {message}")]
    ValidationError { message: String },

    /// Directory access error (e.g., home directory)
    #[error("Directory access error: {source}")]
    DirectoryError {
        #[source]
        source: std::io::Error,
    },

    /// Environment variable error
    #[error("Environment variable error: {message}")]
    EnvironmentError { message: String },
}

impl ConfigError {
    /// Create a new file not found error
    pub fn file_not_found(path: PathBuf) -> Self {
        Self::FileNotFound { path }
    }

    /// Create a new file read error
    pub fn file_read_error(path: PathBuf, source: std::io::Error) -> Self {
        Self::FileReadError { path, source }
    }

    /// Create a new parse error
    pub fn parse_error(path: Option<PathBuf>, source: figment::Error) -> Self {
        Self::ParseError { path, source }
    }

    /// Create a new validation error
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
        }
    }

    /// Create a new directory error
    pub fn directory_error(source: std::io::Error) -> Self {
        Self::DirectoryError { source }
    }

    /// Create a new environment error
    pub fn environment_error(message: impl Into<String>) -> Self {
        Self::EnvironmentError {
            message: message.into(),
        }
    }
}

/// Result type for configuration operations
pub type ConfigResult<T> = Result<T, ConfigError>;
