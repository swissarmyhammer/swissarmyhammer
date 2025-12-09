//! Shared error types and traits for consistent error handling across crates

use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;

/// Category of error for consistent handling and routing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// User input or configuration error - can be fixed by user
    User,
    /// System resource or environmental error - may be temporary
    System,
    /// Internal logic error - indicates a bug
    Internal,
    /// Network or external service error - may be retriable
    External,
}

/// Trait for all errors in the llama-agent ecosystem
///
/// This trait provides a consistent interface for error handling
/// and allows for better error categorization and user experience.
pub trait LlamaError: std::error::Error + Send + Sync + Debug {
    /// Get the error category for proper handling
    fn category(&self) -> ErrorCategory;

    /// Get a unique error code for this error type
    fn error_code(&self) -> &'static str;

    /// Check if this is a user-correctable error
    fn is_user_error(&self) -> bool {
        matches!(self.category(), ErrorCategory::User)
    }

    /// Check if this error is potentially retriable
    fn is_retriable(&self) -> bool {
        matches!(
            self.category(),
            ErrorCategory::System | ErrorCategory::External
        )
    }

    /// Get a user-friendly error message with actionable advice
    fn user_friendly_message(&self) -> String {
        format!("{}", self)
    }

    /// Get suggested recovery actions for this error
    fn recovery_suggestions(&self) -> Vec<String> {
        match self.category() {
            ErrorCategory::User => vec![
                "Check your input parameters".to_string(),
                "Review configuration settings".to_string(),
            ],
            ErrorCategory::System => vec![
                "Check system resources (memory, disk space)".to_string(),
                "Retry the operation".to_string(),
            ],
            ErrorCategory::External => vec![
                "Check network connectivity".to_string(),
                "Verify external service availability".to_string(),
                "Retry after a brief delay".to_string(),
            ],
            ErrorCategory::Internal => vec![
                "Report this as a bug".to_string(),
                "Include error details and reproduction steps".to_string(),
            ],
        }
    }

    /// Get custom retry delay for this specific error instance
    /// Returns None to use default exponential backoff
    fn custom_retry_delay(&self, _attempt: u32) -> Option<Duration> {
        None
    }

    /// Check if retrying should stop regardless of attempt count
    /// Useful for errors like rate limiting where immediate retry is harmful
    fn should_stop_retrying(&self, _attempt: u32) -> bool {
        false
    }
}

/// Base error type that can be used by any crate
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Resource error: {message}")]
    Resource { message: String },

    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl LlamaError for CommonError {
    fn category(&self) -> ErrorCategory {
        match self {
            CommonError::Configuration { .. } => ErrorCategory::User,
            CommonError::Validation { .. } => ErrorCategory::User,
            CommonError::Resource { .. } => ErrorCategory::System,
            CommonError::Network { .. } => ErrorCategory::External,
            CommonError::Internal { .. } => ErrorCategory::Internal,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            CommonError::Configuration { .. } => "COMMON_CONFIG",
            CommonError::Validation { .. } => "COMMON_VALIDATION",
            CommonError::Resource { .. } => "COMMON_RESOURCE",
            CommonError::Network { .. } => "COMMON_NETWORK",
            CommonError::Internal { .. } => "COMMON_INTERNAL",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            CommonError::Configuration { message } => {
                format!("Configuration Error: {}\n💡 Please check your configuration settings and ensure all required values are provided.", message)
            }
            CommonError::Validation { message } => {
                format!(
                    "Validation Error: {}\n💡 Please verify your input parameters and try again.",
                    message
                )
            }
            CommonError::Resource { message } => {
                format!("Resource Error: {}\n💡 Check available system resources (memory, disk space, file permissions).", message)
            }
            CommonError::Network { message } => {
                format!("Network Error: {}\n💡 Check your internet connection and retry. If the problem persists, the remote service may be unavailable.", message)
            }
            CommonError::Internal { message } => {
                format!("Internal Error: {}\n💡 This appears to be a bug. Please report this issue with the error details.", message)
            }
        }
    }
}

/// Result type alias for operations that can return llama errors
pub type LlamaResult<T, E = CommonError> = Result<T, E>;

/// Convenience functions for creating common errors
impl CommonError {
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    pub fn resource<S: Into<String>>(message: S) -> Self {
        Self::Resource {
            message: message.into(),
        }
    }

    pub fn network<S: Into<String>>(message: S) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let config_error = CommonError::configuration("invalid setting");
        assert_eq!(config_error.category(), ErrorCategory::User);
        assert!(config_error.is_user_error());
        assert!(!config_error.is_retriable());

        let resource_error = CommonError::resource("out of memory");
        assert_eq!(resource_error.category(), ErrorCategory::System);
        assert!(!resource_error.is_user_error());
        assert!(resource_error.is_retriable());

        let network_error = CommonError::network("connection failed");
        assert_eq!(network_error.category(), ErrorCategory::External);
        assert!(network_error.is_retriable());

        let internal_error = CommonError::internal("null pointer");
        assert_eq!(internal_error.category(), ErrorCategory::Internal);
        assert!(!internal_error.is_user_error());
        assert!(!internal_error.is_retriable());
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            CommonError::configuration("test").error_code(),
            "COMMON_CONFIG"
        );
        assert_eq!(
            CommonError::validation("test").error_code(),
            "COMMON_VALIDATION"
        );
        assert_eq!(
            CommonError::resource("test").error_code(),
            "COMMON_RESOURCE"
        );
        assert_eq!(CommonError::network("test").error_code(), "COMMON_NETWORK");
        assert_eq!(
            CommonError::internal("test").error_code(),
            "COMMON_INTERNAL"
        );
    }

    #[test]
    fn test_user_friendly_messages() {
        let config_error = CommonError::configuration("missing api key");
        let message = config_error.user_friendly_message();
        assert!(message.contains("Configuration Error"));
        assert!(message.contains("💡"));
        assert!(message.contains("missing api key"));
    }

    #[test]
    fn test_recovery_suggestions() {
        let config_error = CommonError::configuration("test");
        let suggestions = config_error.recovery_suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.contains("configuration")));

        let network_error = CommonError::network("test");
        let suggestions = network_error.recovery_suggestions();
        assert!(suggestions.iter().any(|s| s.contains("network")));
    }
}
