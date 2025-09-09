//! Error types for the templating domain

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
