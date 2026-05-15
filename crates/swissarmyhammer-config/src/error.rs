use std::path::PathBuf;
use swissarmyhammer_common::{ErrorSeverity, Severity};
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

impl Severity for ConfigurationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Core configuration system failures
            Self::LoadError { .. } => ErrorSeverity::Critical,
            Self::FigmentError { .. } => ErrorSeverity::Critical,

            // Error: Configuration issues that affect functionality but allow fallback
            Self::DiscoveryError { .. } => ErrorSeverity::Error,
            Self::EnvVarError { .. } => ErrorSeverity::Error,
            Self::TemplateContextError { .. } => ErrorSeverity::Error,
            Self::IoError { .. } => ErrorSeverity::Error,
            Self::JsonError { .. } => ErrorSeverity::Error,
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;
    use std::io;

    #[test]
    fn test_load_error_is_critical() {
        let error = ConfigurationError::load(
            PathBuf::from("/test/config.yaml"),
            figment::error::Kind::MissingField("test".into()).into(),
        );
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_figment_error_is_critical() {
        let figment_error: figment::Error =
            figment::error::Kind::MissingField("test".into()).into();
        let error = ConfigurationError::from(figment_error);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_discovery_error_is_error() {
        let error = ConfigurationError::discovery("Failed to discover config");
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_env_var_error_is_error() {
        let error = ConfigurationError::env_var("Missing env var");
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_template_context_error_is_error() {
        let error = ConfigurationError::template_context("Template error");
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_io_error_is_error() {
        let error =
            ConfigurationError::from(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_json_error_is_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let error = ConfigurationError::from(json_err);
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }
}
