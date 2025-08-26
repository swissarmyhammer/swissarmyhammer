use std::path::PathBuf;
use thiserror::Error;

/// Result type for configuration operations
pub type ConfigurationResult<T> = Result<T, ConfigurationError>;

/// Configuration error types
#[derive(Error, Debug)]
pub enum ConfigurationError {
    /// Error loading or parsing configuration file
    #[error("Failed to load configuration from {path}: {source}")]
    LoadError {
        path: PathBuf,
        #[source]
        source: Box<figment::Error>,
    },

    /// Error during file discovery
    #[error("Configuration file discovery failed: {message}")]
    DiscoveryError { message: String },

    /// Error during environment variable substitution
    #[error("Environment variable substitution failed: {message}")]
    EnvVarError { message: String },

    /// Error during template context operations
    #[error("Template context error: {message}")]
    TemplateContextError { message: String },

    /// IO error during configuration operations
    #[error("IO error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    /// JSON serialization/deserialization error
    #[error("JSON error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },

    /// Figment configuration error
    #[error("Figment error: {source}")]
    FigmentError {
        #[source]
        source: Box<figment::Error>,
    },
}

impl ConfigurationError {
    /// Create a new discovery error
    pub fn discovery(message: impl Into<String>) -> Self {
        Self::DiscoveryError {
            message: message.into(),
        }
    }

    /// Create a new environment variable error
    pub fn env_var(message: impl Into<String>) -> Self {
        Self::EnvVarError {
            message: message.into(),
        }
    }

    /// Create a new template context error
    pub fn template_context(message: impl Into<String>) -> Self {
        Self::TemplateContextError {
            message: message.into(),
        }
    }

    /// Create a new load error with path context
    pub fn load(path: PathBuf, source: figment::Error) -> Self {
        Self::LoadError {
            path,
            source: Box::new(source),
        }
    }
}

impl From<figment::Error> for ConfigurationError {
    fn from(error: figment::Error) -> Self {
        Self::FigmentError {
            source: Box::new(error),
        }
    }
}
