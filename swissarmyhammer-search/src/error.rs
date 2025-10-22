//! Error types for SwissArmyHammer search operations

use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Search specific errors
#[derive(Error, Debug)]
pub enum SearchError {
    /// Database operation failed
    #[error("Database error: {0}")]
    Database(String),

    /// Storage operation failed
    #[error("Storage error: {0}")]
    Storage(String),

    /// Vector storage operation failed
    #[error("Vector storage operation failed: {operation}")]
    VectorStorage {
        /// The operation that failed
        operation: String,
        /// The underlying storage error
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Embedding generation failed
    #[error("Embedding error: {0}")]
    Embedding(String),

    /// File system operation failed
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    /// IO operation failed (alias for FileSystem)
    #[error("IO error: {0}")]
    Io(std::io::Error),

    /// TreeSitter parsing failed
    #[error("TreeSitter parsing error: {0}")]
    TreeSitter(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// Serialization or deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// ONNX Runtime error
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(#[from] ort::Error),

    /// Index operation failed
    #[error("Index error: {0}")]
    Index(String),

    /// Search operation failed with context
    #[error("Search failed during {operation}: {message}")]
    SearchOperation {
        /// The search operation that failed
        operation: String,
        /// Descriptive error message
        message: String,
        /// The underlying error if available
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Generic search error (deprecated - use SearchOperation instead)
    #[error("Search error: {0}")]
    Search(String),

    /// Semantic search error
    #[error("Semantic error: {0}")]
    Semantic(String),
}

/// Result type for search operations
pub type SearchResult<T> = std::result::Result<T, SearchError>;

impl Severity for SearchError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Search infrastructure unavailable - system cannot continue
            SearchError::Database(_) => ErrorSeverity::Critical,
            SearchError::VectorStorage { .. } => ErrorSeverity::Critical,
            SearchError::OnnxRuntime(_) => ErrorSeverity::Critical,

            // Error: Operation failed but system can continue
            SearchError::Storage(_) => ErrorSeverity::Error,
            SearchError::Embedding(_) => ErrorSeverity::Error,
            SearchError::TreeSitter(_) => ErrorSeverity::Error,
            SearchError::Config(_) => ErrorSeverity::Error,
            SearchError::Serialization(_) => ErrorSeverity::Error,
            SearchError::Index(_) => ErrorSeverity::Error,
            SearchError::SearchOperation { .. } => ErrorSeverity::Error,
            SearchError::Search(_) => ErrorSeverity::Error,
            SearchError::Semantic(_) => ErrorSeverity::Error,

            // Warning: Non-critical file system issues
            SearchError::FileSystem(_) => ErrorSeverity::Warning,
            SearchError::Io(_) => ErrorSeverity::Warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_error_critical_severity() {
        let error = SearchError::Database("connection failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);

        let error = SearchError::VectorStorage {
            operation: "insert".to_string(),
            source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, "test")),
        };
        assert_eq!(error.severity(), ErrorSeverity::Critical);

        let error = SearchError::OnnxRuntime(ort::Error::new("test error"));
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_search_error_error_severity() {
        let error = SearchError::Storage("write failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Embedding("model failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::TreeSitter("parse failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Config("invalid config".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Serialization(
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
        );
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Index("index failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::SearchOperation {
            operation: "query".to_string(),
            message: "search failed".to_string(),
            source: None,
        };
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Search("generic error".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = SearchError::Semantic("semantic error".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_search_error_warning_severity() {
        let error = SearchError::FileSystem(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        let error = SearchError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
