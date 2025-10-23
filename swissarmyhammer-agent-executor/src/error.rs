//! Error types for agent execution
//!
//! This module defines the error hierarchy used throughout the agent executor crate.
//! All executor operations return [`ActionResult<T>`] which is an alias for `Result<T, ActionError>`.
//!
//! # Error Hierarchy
//!
//! The [`ActionError`] enum represents all possible errors during agent execution:
//!
//! - **ClaudeError**: Claude-specific execution failures (model unavailable, API errors)
//! - **VariableError**: Variable resolution or substitution failures
//! - **ParseError**: Prompt or response parsing failures
//! - **ExecutionError**: Generic execution failures (initialization, generation errors)
//! - **IoError**: File system or I/O failures
//! - **JsonError**: JSON serialization/deserialization failures
//! - **RateLimit**: Rate limiting errors with retry timing information
//!
//! # When to Use ActionError vs ActionResult
//!
//! - **ActionError**: Use when creating or returning errors explicitly
//! - **ActionResult<T>**: Use as the return type for fallible operations
//!
//! # Error Conversion
//!
//! Several error types implement `From` traits for automatic conversion:
//!
//! - `std::io::Error` → `ActionError::IoError`
//! - `serde_json::Error` → `ActionError::JsonError`
//!
//! # Rate Limiting
//!
//! The `RateLimit` variant includes retry timing information to help callers
//! implement exponential backoff and respect API rate limits.
//!
//! # Usage
//!
//! ```rust
//! use swissarmyhammer_agent_executor::{ActionError, ActionResult};
//!
//! fn process_prompt(prompt: &str) -> ActionResult<String> {
//!     if prompt.is_empty() {
//!         return Err(ActionError::ParseError("Empty prompt".to_string()));
//!     }
//!     Ok("Success".to_string())
//! }
//! ```

use std::time::Duration;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Errors that can occur during action execution
#[derive(Debug, Error)]
pub enum ActionError {
    /// Claude command execution failed
    #[error("Claude execution failed: {0}")]
    ClaudeError(String),
    /// Variable operation failed
    #[error("Variable operation failed: {0}")]
    VariableError(String),
    /// Action parsing failed
    #[error("Action parsing failed: {0}")]
    ParseError(String),
    /// Generic action execution error
    #[error("Action execution failed: {0}")]
    ExecutionError(String),
    /// IO error during action execution
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Rate limit error with retry time
    #[error("Rate limit reached. Please wait {wait_time:?} and try again. Details: {message}")]
    RateLimit {
        /// The error message
        message: String,
        /// How long to wait before retrying
        wait_time: Duration,
    },
}

/// Result type for action operations
pub type ActionResult<T> = std::result::Result<T, ActionError>;

/// Implementation of Severity trait for ActionError
///
/// This implementation categorizes all ActionError variants by their
/// severity level to enable appropriate error handling, logging, and user notification.
///
/// # Severity Assignment Guidelines
///
/// - **Critical**: Agent system cannot function, model unavailable
///   - ClaudeError: Model or API unavailable prevents agent execution
///
/// - **Error**: Operation-specific failures that are not recoverable
///   - VariableError: Variable resolution failed
///   - ParseError: Action or response parsing failed
///   - ExecutionError: Action execution failed
///   - IoError: File system operations failed
///   - JsonError: JSON serialization/deserialization failed
///
/// - **Warning**: Temporary issues that can be retried
///   - RateLimit: Rate limiting with retry timing information
impl Severity for ActionError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Claude model/API unavailable - agent cannot function
            ActionError::ClaudeError(_) => ErrorSeverity::Critical,

            // Error: Operation failed but system can continue
            ActionError::VariableError(_) => ErrorSeverity::Error,
            ActionError::ParseError(_) => ErrorSeverity::Error,
            ActionError::ExecutionError(_) => ErrorSeverity::Error,
            ActionError::IoError(_) => ErrorSeverity::Error,
            ActionError::JsonError(_) => ErrorSeverity::Error,

            // Warning: Temporary issue, can retry after wait time
            ActionError::RateLimit { .. } => ErrorSeverity::Warning,
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_claude_error_is_critical() {
        let error = ActionError::ClaudeError("model unavailable".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_variable_error_is_error_level() {
        let error = ActionError::VariableError("variable not found".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_parse_error_is_error_level() {
        let error = ActionError::ParseError("invalid format".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_execution_error_is_error_level() {
        let error = ActionError::ExecutionError("execution failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_io_error_is_error_level() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = ActionError::IoError(io_err);
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_json_error_is_error_level() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json")
            .unwrap_err();
        let error = ActionError::JsonError(json_err);
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_rate_limit_error_is_warning() {
        let error = ActionError::RateLimit {
            message: "rate limit exceeded".to_string(),
            wait_time: Duration::from_secs(60),
        };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
