//! Error types for the templating domain

use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Result type for templating operations
pub type Result<T> = std::result::Result<T, TemplatingError>;

/// Errors that can occur during template operations
#[derive(Error, Debug)]
pub enum TemplatingError {
    /// Template parsing or compilation failed
    #[error("Template parsing error: {0}")]
    Parse(String),

    /// Template rendering failed
    #[error("Template rendering error: {0}")]
    Render(String),

    /// Security validation failed
    #[error("Template security validation failed: {0}")]
    Security(String),

    /// Partial template loading failed
    #[error("Partial template error: {0}")]
    Partial(String),

    /// Template variable extraction failed
    #[error("Variable extraction error: {0}")]
    VariableExtraction(String),

    /// Template timeout during rendering
    #[error("Template rendering timed out after {timeout_ms}ms")]
    Timeout {
        /// The timeout duration in milliseconds
        timeout_ms: u64,
    },

    /// IO error during template operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error for unexpected conditions
    #[error("Template error: {0}")]
    Other(String),
}

impl From<liquid::Error> for TemplatingError {
    fn from(err: liquid::Error) -> Self {
        TemplatingError::Render(err.to_string())
    }
}

impl From<anyhow::Error> for TemplatingError {
    fn from(err: anyhow::Error) -> Self {
        TemplatingError::Other(err.to_string())
    }
}

impl From<String> for TemplatingError {
    fn from(err: String) -> Self {
        TemplatingError::Other(err)
    }
}

impl From<&str> for TemplatingError {
    fn from(err: &str) -> Self {
        TemplatingError::Other(err.to_string())
    }
}

/// Implementation of Severity trait for TemplatingError
///
/// This implementation categorizes all TemplatingError variants by their
/// severity level to enable appropriate error handling, logging, and user notification.
///
/// # Severity Assignment Guidelines
///
/// - **Error**: All template errors prevent successful template operations
///   - Parse errors: Template syntax is invalid
///   - Render errors: Template rendering failed
///   - Security errors: Unsafe operations blocked
///   - Partial errors: Partial template loading failed
///   - Variable extraction errors: Cannot extract template variables
///   - Timeout errors: Template rendering took too long
///   - I/O errors: File system operations failed
///   - JSON errors: Serialization/deserialization failed
///   - Other errors: Unexpected template failures
impl Severity for TemplatingError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // All template errors are operation-level failures
            // They prevent completing the specific template operation
            // but the system remains stable
            TemplatingError::Parse(_) => ErrorSeverity::Error,
            TemplatingError::Render(_) => ErrorSeverity::Error,
            TemplatingError::Security(_) => ErrorSeverity::Error,
            TemplatingError::Partial(_) => ErrorSeverity::Error,
            TemplatingError::VariableExtraction(_) => ErrorSeverity::Error,
            TemplatingError::Timeout { .. } => ErrorSeverity::Error,
            TemplatingError::Io(_) => ErrorSeverity::Error,
            TemplatingError::Json(_) => ErrorSeverity::Error,
            TemplatingError::Other(_) => ErrorSeverity::Error,
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_parse_error_is_error_level() {
        let error = TemplatingError::Parse("invalid syntax".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_render_error_is_error_level() {
        let error = TemplatingError::Render("render failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_security_error_is_error_level() {
        let error = TemplatingError::Security("unsafe operation".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_partial_error_is_error_level() {
        let error = TemplatingError::Partial("partial not found".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_variable_extraction_error_is_error_level() {
        let error = TemplatingError::VariableExtraction("failed to extract".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_timeout_error_is_error_level() {
        let error = TemplatingError::Timeout { timeout_ms: 5000 };
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_io_error_is_error_level() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = TemplatingError::Io(io_err);
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_json_error_is_error_level() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json")
            .unwrap_err();
        let error = TemplatingError::Json(json_err);
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_other_error_is_error_level() {
        let error = TemplatingError::Other("unexpected error".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }
}
