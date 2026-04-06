//! Error types for the views registry

use thiserror::Error;

/// Result type for views operations
pub type Result<T> = std::result::Result<T, ViewsError>;

/// Errors that can occur in view registry operations
#[derive(Debug, Error)]
pub enum ViewsError {
    /// View not found by id
    #[error("view not found: {id}")]
    ViewNotFound { id: String },

    /// View not found by name
    #[error("view not found by name: {name}")]
    ViewNotFoundByName { name: String },

    /// Duplicate view id
    #[error("duplicate view id: {id}")]
    DuplicateViewId { id: String },

    /// Changelog entry not found
    #[error("changelog entry not found: {id}")]
    ChangelogEntryNotFound { id: String },

    /// Nothing to undo
    #[error("nothing to undo")]
    NothingToUndo,

    /// Nothing to redo
    #[error("nothing to redo")]
    NothingToRedo,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization error
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = ViewsError::ViewNotFound { id: "01ABC".into() };
        assert_eq!(err.to_string(), "view not found: 01ABC");
    }

    /// Cover all error variant Display implementations.
    #[test]
    fn error_display_all_variants() {
        let err = ViewsError::ViewNotFoundByName {
            name: "Board".into(),
        };
        assert_eq!(err.to_string(), "view not found by name: Board");

        let err = ViewsError::DuplicateViewId { id: "01DUP".into() };
        assert_eq!(err.to_string(), "duplicate view id: 01DUP");

        let err = ViewsError::ChangelogEntryNotFound {
            id: "01ENTRY".into(),
        };
        assert_eq!(err.to_string(), "changelog entry not found: 01ENTRY");

        let err = ViewsError::NothingToUndo;
        assert_eq!(err.to_string(), "nothing to undo");

        let err = ViewsError::NothingToRedo;
        assert_eq!(err.to_string(), "nothing to redo");
    }

    /// Cover From<std::io::Error> conversion.
    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: ViewsError = io_err.into();
        assert!(err.to_string().contains("file missing"));
    }

    /// Cover From<serde_json::Error> conversion.
    #[test]
    fn error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: ViewsError = json_err.into();
        assert!(err.to_string().contains("JSON error"));
    }

    /// Cover From<serde_yaml_ng::Error> conversion.
    #[test]
    fn error_from_yaml() {
        let yaml_err = serde_yaml_ng::from_str::<serde_yaml_ng::Value>("- :\n  - [").unwrap_err();
        let err: ViewsError = yaml_err.into();
        assert!(err.to_string().contains("YAML error"));
    }
}
