//! Error types for memoranda operations

use thiserror::Error;

/// Result type alias for memoranda operations
pub type Result<T> = std::result::Result<T, MemorandaError>;

/// Errors that can occur during memoranda operations
#[derive(Error, Debug)]
pub enum MemorandaError {
    #[error("Memo not found: {title}")]
    MemoNotFound { title: String },

    #[error("Invalid memo title: {0}")]
    InvalidTitle(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}