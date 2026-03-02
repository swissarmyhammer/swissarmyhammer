//! Error types for the fields registry

use std::path::PathBuf;
use thiserror::Error;

/// Result type for fields operations
pub type Result<T> = std::result::Result<T, FieldsError>;

/// Errors that can occur in field registry operations
#[derive(Debug, Error)]
pub enum FieldsError {
    /// Field not found by name
    #[error("field not found: {name}")]
    FieldNotFound { name: String },

    /// Field not found by ULID
    #[error("field not found by id: {id}")]
    FieldNotFoundById { id: String },

    /// Entity template not found
    #[error("entity template not found: {name}")]
    EntityNotFound { name: String },

    /// Duplicate field name
    #[error("duplicate field name: {name}")]
    DuplicateFieldName { name: String },

    /// Validation error (JS validation function threw)
    #[error("validation error on field '{field}': {message}")]
    ValidationFailed { field: String, message: String },

    /// Fields directory not found
    #[error("fields directory not found: {path}")]
    NotInitialized { path: PathBuf },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization error
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FieldsError::FieldNotFound {
            name: "status".into(),
        };
        assert_eq!(err.to_string(), "field not found: status");
    }

    #[test]
    fn test_validation_error() {
        let err = FieldsError::ValidationFailed {
            field: "tag_name".into(),
            message: "cannot be empty".into(),
        };
        assert!(err.to_string().contains("tag_name"));
        assert!(err.to_string().contains("cannot be empty"));
    }
}
