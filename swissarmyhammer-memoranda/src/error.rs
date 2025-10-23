//! Error types for memoranda operations

use swissarmyhammer_common::{ErrorSeverity, Severity};
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

impl Severity for MemorandaError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Storage system failed - cannot continue
            MemorandaError::Storage(_) => ErrorSeverity::Critical,

            // Error: Specific operation failed but system can continue
            MemorandaError::MemoNotFound { .. } => ErrorSeverity::Error,
            MemorandaError::InvalidTitle(_) => ErrorSeverity::Error,
            MemorandaError::Serialization(_) => ErrorSeverity::Error,
            MemorandaError::InvalidOperation(_) => ErrorSeverity::Error,
            MemorandaError::Io(_) => ErrorSeverity::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memoranda_error_critical_severity() {
        let error = MemorandaError::Storage("storage system failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_memoranda_error_error_severity() {
        let error = MemorandaError::MemoNotFound {
            title: "test".to_string(),
        };
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = MemorandaError::InvalidTitle("invalid title".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = MemorandaError::Serialization("serialization failed".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = MemorandaError::InvalidOperation("invalid operation".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let error = MemorandaError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }
}
