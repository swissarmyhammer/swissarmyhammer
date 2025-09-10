//! Error types for SwissArmyHammer Common
//!
//! This module provides structured error handling for common operations
//! throughout the SwissArmyHammer ecosystem. This includes core infrastructure
//! errors that are shared across all SwissArmyHammer crates.

use std::fmt;
use std::io;
use std::path::PathBuf;
use thiserror::Error as ThisError;

/// Result type alias for SwissArmyHammer operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Common error types for SwissArmyHammer operations
///
/// This enum contains core infrastructure errors that are shared across
/// the SwissArmyHammer ecosystem. Domain-specific errors should be defined
/// in their respective crates and converted to these common types as needed.
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum SwissArmyHammerError {
    /// IO operation failed
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_yaml::Error),

    /// Workflow not found
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    /// Workflow run not found
    #[error("Workflow run not found: {0}")]
    WorkflowRunNotFound(String),

    /// Storage backend error
    #[error("Storage error: {0}")]
    Storage(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// File not found
    #[error("File not found: {path}\nSuggestion: {suggestion}")]
    FileNotFound {
        /// The file path that was not found
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Path is not a file (e.g., directory)
    #[error("Path is not a file: {path}\nSuggestion: {suggestion}")]
    NotAFile {
        /// The path that is not a file
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Permission denied when accessing file
    #[error("Permission denied accessing file: {path}\nError: {error}\nSuggestion: {suggestion}")]
    PermissionDenied {
        /// The file path that could not be accessed
        path: String,
        /// The underlying error message
        error: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Invalid file path format
    #[error("Invalid file path: {path}\nSuggestion: {suggestion}")]
    InvalidFilePath {
        /// The invalid file path
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// SwissArmyHammer must be run from within a Git repository
    #[error("SwissArmyHammer must be run from within a Git repository")]
    NotInGitRepository,

    /// Failed to create .swissarmyhammer directory
    #[error("Failed to create .swissarmyhammer directory: {0}")]
    DirectoryCreation(String),

    /// Git repository found but .swissarmyhammer directory is not accessible
    #[error("Git repository found but .swissarmyhammer directory is not accessible: {0}")]
    DirectoryAccess(String),

    /// Invalid path encountered
    #[error("Invalid path: {path}")]
    InvalidPath {
        /// The invalid path that caused the error
        path: PathBuf,
    },

    /// General I/O error with context
    #[error("I/O error: {message}")]
    IoContext {
        /// Descriptive message about the I/O error
        message: String,
    },

    /// Semantic search related error
    #[error("Semantic search error: {message}")]
    Semantic {
        /// Error message from semantic search operations
        message: String,
    },

    /// Generic error with context
    #[error("{message}")]
    Context {
        /// The error message providing context
        message: String,
        #[source]
        /// The underlying error that caused this error
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Other error with custom message
    #[error("{message}")]
    Other {
        /// Custom error message
        message: String,
    },
}

impl SwissArmyHammerError {
    /// Create a file not found error with suggestion
    pub fn file_not_found(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::FileNotFound {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a not a file error (for directories) with suggestion
    pub fn not_a_file(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::NotAFile {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a permission denied error with suggestion
    pub fn permission_denied(path: &str, error: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::PermissionDenied {
            path: path.to_string(),
            error: error.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create an invalid file path error with suggestion
    pub fn invalid_file_path(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::InvalidFilePath {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a directory creation error
    pub fn directory_creation(error: std::io::Error) -> Self {
        SwissArmyHammerError::DirectoryCreation(error.to_string())
    }

    /// Create a directory access error
    pub fn directory_access(details: &str) -> Self {
        SwissArmyHammerError::DirectoryAccess(details.to_string())
    }

    /// Create a new invalid path error
    pub fn invalid_path(path: PathBuf) -> Self {
        Self::InvalidPath { path }
    }

    /// Create a new I/O error with context
    pub fn io_context(message: String) -> Self {
        Self::IoContext { message }
    }

    /// Create a new semantic search error
    pub fn semantic(message: String) -> Self {
        Self::Semantic { message }
    }

    /// Create a new other error
    pub fn other(message: String) -> Self {
        Self::Other { message }
    }
}

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context<S: Into<String>>(self, msg: S) -> Result<T>;

    /// Add context with a closure that's only called on error
    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context<S: Into<String>>(self, msg: S) -> Result<T> {
        self.map_err(|e| SwissArmyHammerError::Context {
            message: msg.into(),
            source: Box::new(e),
        })
    }

    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| SwissArmyHammerError::Context {
            message: f().into(),
            source: Box::new(e),
        })
    }
}

/// Error chain formatter for detailed error reporting
pub struct ErrorChain<'a>(&'a dyn std::error::Error);

impl fmt::Display for ErrorChain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Error: {}", self.0)?;

        let mut current = self.0.source();
        let mut level = 1;

        while let Some(err) = current {
            writeln!(f, "{:indent$}Caused by: {}", "", err, indent = level * 2)?;
            current = err.source();
            level += 1;
        }

        Ok(())
    }
}

/// Extension trait for error types to format the full error chain
pub trait ErrorChainExt {
    /// Format the full error chain
    fn error_chain(&self) -> ErrorChain<'_>;
}

impl<E: std::error::Error> ErrorChainExt for E {
    fn error_chain(&self) -> ErrorChain<'_> {
        ErrorChain(self)
    }
}
