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
    Yaml(#[from] serde_yaml::Error),

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
}
