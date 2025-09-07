//! Error types for SwissArmyHammer search operations

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

// Note: SwissArmyHammerError conversion removed as it doesn't exist in swissarmyhammer_common