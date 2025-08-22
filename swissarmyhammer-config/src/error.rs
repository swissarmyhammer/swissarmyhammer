//! Error types for SwissArmyHammer configuration system

use std::path::PathBuf;
use thiserror::Error;

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: PathBuf },

    /// Failed to read configuration file
    #[error("Failed to read configuration file {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Configuration parsing failed
    #[error("Failed to parse configuration: {source}")]
    ParseError { source: figment::Error },

    /// Configuration validation failed
    #[error("Configuration validation failed: {message}")]
    ValidationError { message: String },

    /// Home directory could not be determined
    #[error("Unable to determine home directory")]
    HomeDirectoryNotFound,

    /// Current directory could not be determined
    #[error("Unable to determine current directory")]
    CurrentDirectoryNotFound,

    /// Git repository root could not be found
    #[error("Git repository root not found")]
    GitRootNotFound,

    /// Invalid configuration value
    #[error("Invalid configuration value for key '{key}': {message}")]
    InvalidValue { key: String, message: String },

    /// Configuration file format not supported
    #[error("Unsupported configuration file format: {format}")]
    UnsupportedFormat { format: String },
}

impl From<figment::Error> for ConfigError {
    fn from(error: figment::Error) -> Self {
        ConfigError::ParseError { source: error }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => ConfigError::CurrentDirectoryNotFound,
            _ => ConfigError::FileRead {
                path: PathBuf::new(),
                source: error,
            },
        }
    }
}
