//! Error types for the perspectives registry

use thiserror::Error;

/// Result type for perspective operations
pub type Result<T> = std::result::Result<T, PerspectiveError>;

/// Errors that can occur in perspective registry operations
#[derive(Debug, Error)]
pub enum PerspectiveError {
    /// Resource not found by id
    #[error("{resource} not found: {id}")]
    NotFound { resource: String, id: String },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization error
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Store layer error (from StoreHandle write/delete)
    #[error("Store error: {0}")]
    Store(#[from] swissarmyhammer_store::StoreError),
}

impl PerspectiveError {
    /// Create a NotFound error
    pub fn not_found(resource: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
            id: id.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = PerspectiveError::not_found("perspective", "01ABC");
        assert_eq!(err.to_string(), "perspective not found: 01ABC");
    }

    #[test]
    fn not_found_struct() {
        let err = PerspectiveError::NotFound {
            resource: "field".into(),
            id: "xyz".into(),
        };
        assert_eq!(err.to_string(), "field not found: xyz");
    }

    #[test]
    fn io_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: PerspectiveError = io_err.into();
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn result_type_alias() {
        let ok: Result<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: Result<i32> = Err(PerspectiveError::not_found("test", "1"));
        assert!(err.is_err());
    }
}
