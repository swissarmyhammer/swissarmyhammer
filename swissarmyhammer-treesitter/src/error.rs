//! Error types for SwissArmyHammer Tree-sitter
//!
//! This module provides structured error handling for tree-sitter parsing,
//! indexing, and file watching operations.

use std::path::PathBuf;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error as ThisError;

/// Result type alias for tree-sitter operations
pub type Result<T> = std::result::Result<T, TreeSitterError>;

/// Error types for tree-sitter operations
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum TreeSitterError {
    /// Unsupported language for file
    #[error("Unsupported language for file: {path}")]
    UnsupportedLanguage {
        /// The file path
        path: PathBuf,
        /// The file extension if available
        extension: Option<String>,
    },

    /// Parse error occurred
    #[error("Parse error in {path}: {message}")]
    ParseError {
        /// The file path
        path: PathBuf,
        /// Error message
        message: String,
    },

    /// Parse operation timed out
    #[error("Parse timeout after {timeout_ms}ms for {path}")]
    ParseTimeout {
        /// The file path
        path: PathBuf,
        /// Timeout in milliseconds
        timeout_ms: u64,
    },

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Index lock error
    #[error("Index lock error: {0}")]
    LockError(String),

    /// File watcher error
    #[error("Watcher error: {0}")]
    WatcherError(String),

    /// Query compilation error
    #[error("Query compilation error for {language}: {message}")]
    QueryError {
        /// The language name
        language: String,
        /// Error message
        message: String,
    },

    /// File too large to parse
    #[error("File too large: {path} ({size} bytes, max {max_size} bytes)")]
    FileTooLarge {
        /// The file path
        path: PathBuf,
        /// Actual file size
        size: u64,
        /// Maximum allowed size
        max_size: u64,
    },

    /// Index not initialized
    #[error("Index not initialized. Call initialize() first.")]
    NotInitialized,

    /// Embedding model error
    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    /// Connection error (e.g., failed to connect to index leader)
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

impl Severity for TreeSitterError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Warning: Recoverable issues that don't prevent operation
            TreeSitterError::UnsupportedLanguage { .. } => ErrorSeverity::Warning,
            TreeSitterError::ParseError { .. } => ErrorSeverity::Warning,
            TreeSitterError::ParseTimeout { .. } => ErrorSeverity::Warning,
            TreeSitterError::FileTooLarge { .. } => ErrorSeverity::Warning,

            // Error: Operation failed but system can continue
            TreeSitterError::FileNotFound(_) => ErrorSeverity::Error,
            TreeSitterError::Io(_) => ErrorSeverity::Error,
            TreeSitterError::WatcherError(_) => ErrorSeverity::Error,
            TreeSitterError::NotInitialized => ErrorSeverity::Error,

            // Critical: System-level failures
            TreeSitterError::LockError(_) => ErrorSeverity::Critical,
            TreeSitterError::QueryError { .. } => ErrorSeverity::Critical,
            TreeSitterError::EmbeddingError(_) => ErrorSeverity::Error,
            TreeSitterError::ConnectionError(_) => ErrorSeverity::Error,
        }
    }
}

impl TreeSitterError {
    /// Create an unsupported language error
    pub fn unsupported_language(path: PathBuf) -> Self {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());
        TreeSitterError::UnsupportedLanguage { path, extension }
    }

    /// Create a parse error
    pub fn parse_error(path: PathBuf, message: impl Into<String>) -> Self {
        TreeSitterError::ParseError {
            path,
            message: message.into(),
        }
    }

    /// Create a parse timeout error
    pub fn parse_timeout(path: PathBuf, timeout_ms: u64) -> Self {
        TreeSitterError::ParseTimeout { path, timeout_ms }
    }

    /// Create a lock error
    pub fn lock_error(message: impl Into<String>) -> Self {
        TreeSitterError::LockError(message.into())
    }

    /// Create a watcher error
    pub fn watcher_error(message: impl Into<String>) -> Self {
        TreeSitterError::WatcherError(message.into())
    }

    /// Create a query error
    pub fn query_error(language: impl Into<String>, message: impl Into<String>) -> Self {
        TreeSitterError::QueryError {
            language: language.into(),
            message: message.into(),
        }
    }

    /// Create a file too large error
    pub fn file_too_large(path: PathBuf, size: u64, max_size: u64) -> Self {
        TreeSitterError::FileTooLarge {
            path,
            size,
            max_size,
        }
    }

    /// Create an embedding error
    pub fn embedding_error(message: impl Into<String>) -> Self {
        TreeSitterError::EmbeddingError(message.into())
    }

    /// Create a connection error
    pub fn connection_error(message: impl Into<String>) -> Self {
        TreeSitterError::ConnectionError(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_severity_warning() {
        let errors = vec![
            TreeSitterError::unsupported_language(PathBuf::from("test.xyz")),
            TreeSitterError::parse_error(PathBuf::from("test.rs"), "syntax error"),
            TreeSitterError::parse_timeout(PathBuf::from("test.rs"), 5000),
            TreeSitterError::file_too_large(PathBuf::from("test.rs"), 20_000_000, 10_000_000),
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Warning,
                "Expected Warning severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_error_severity_error() {
        let errors: Vec<TreeSitterError> = vec![
            TreeSitterError::FileNotFound(PathBuf::from("test.rs")),
            TreeSitterError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test")),
            TreeSitterError::watcher_error("test error"),
            TreeSitterError::NotInitialized,
            TreeSitterError::embedding_error("model load failed"),
            TreeSitterError::connection_error("failed to connect"),
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Error,
                "Expected Error severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_error_severity_critical() {
        let errors = vec![
            TreeSitterError::lock_error("lock poisoned"),
            TreeSitterError::query_error("rust", "invalid query"),
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Critical,
                "Expected Critical severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_unsupported_language_extracts_extension() {
        let error = TreeSitterError::unsupported_language(PathBuf::from("/path/to/file.xyz"));
        match error {
            TreeSitterError::UnsupportedLanguage { extension, .. } => {
                assert_eq!(extension, Some("xyz".to_string()));
            }
            _ => panic!("Expected UnsupportedLanguage variant"),
        }
    }
}
