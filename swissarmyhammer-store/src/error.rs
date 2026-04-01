//! Error types for the store crate.

use thiserror::Error;

/// Errors that can occur during store operations.
#[derive(Error, Debug)]
pub enum StoreError {
    /// An I/O error occurred reading or writing files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A YAML serialization or deserialization error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    /// The requested item was not found in the store.
    #[error("item not found: {0}")]
    NotFound(String),

    /// The requested changelog entry was not found.
    #[error("changelog entry not found: {0}")]
    EntryNotFound(String),

    /// A patch could not be applied to the target text.
    #[error("patch failed: {0}")]
    PatchFailed(String),

    /// A three-way merge encountered conflicts.
    #[error("merge conflict: {0}")]
    MergeConflict(String),

    /// Failed to deserialize an item from its on-disk representation.
    #[error("deserialization error: {0}")]
    Deserialize(String),

    /// No registered store could handle the given undo entry.
    #[error("no provider found for undo entry: {0}")]
    NoProvider(String),
}

/// Convenience alias for `Result<T, StoreError>`.
pub type Result<T> = std::result::Result<T, StoreError>;
