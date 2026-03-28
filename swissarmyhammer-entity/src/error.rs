//! Error types for the entity crate.

use std::path::PathBuf;
use thiserror::Error;

/// Result type for entity operations.
pub type Result<T> = std::result::Result<T, EntityError>;

/// Errors that can occur in entity operations.
#[derive(Debug, Error)]
pub enum EntityError {
    /// Entity file not found.
    #[error("entity not found: {entity_type}/{id}")]
    NotFound { entity_type: String, id: String },

    /// Missing frontmatter delimiters in a body-field entity.
    #[error("invalid frontmatter in {path}: expected --- delimiters")]
    InvalidFrontmatter { path: PathBuf },

    /// YAML parse error.
    #[error("YAML error in {path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml_ng::Error,
    },

    /// Unknown entity type (not defined in FieldsContext).
    #[error("unknown entity type: {entity_type}")]
    UnknownEntityType { entity_type: String },

    /// Field validation failed.
    #[error("validation failed for field '{field}': {message}")]
    ValidationFailed { field: String, message: String },

    /// Computed field derivation failed.
    #[error("compute error for field '{field}': {message}")]
    ComputeError { field: String, message: String },

    /// A text diff patch could not be parsed or applied.
    #[error("patch apply error: {0}")]
    PatchApply(String),

    /// A non-string field change is stale: the entity's current value does not
    /// match the expected value from the changelog entry.
    #[error("stale change on field '{field}': expected {expected}, found {actual}")]
    StaleChange {
        field: String,
        expected: serde_json::Value,
        actual: serde_json::Value,
    },

    /// An undo or redo was attempted on an unsupported operation type
    /// (e.g. trying to undo an "undo" or "redo" entry directly).
    #[error("unsupported undo/redo operation type: '{op}'")]
    UnsupportedUndoOp { op: String },

    /// A changelog ULID was not found in the index.
    #[error("changelog entry not found: {ulid}")]
    ChangelogEntryNotFound { ulid: String },

    /// Transaction undo/redo failed partway through. Rollback was attempted.
    ///
    /// When a multi-entry transaction undo or redo fails on one entry after
    /// some entries have already been reversed, the system attempts to roll
    /// back the completed entries to restore consistency. This error reports
    /// both the original failure and whether rollback succeeded.
    #[error(
        "transaction partial failure on entry {failed_entry}: {original_error} \
         (completed {completed_count} entries, rollback {rollback_status})",
        completed_count = completed.len(),
        rollback_status = if *rollback_succeeded { "succeeded" } else { "failed" }
    )]
    TransactionPartialFailure {
        /// The original error that caused the failure.
        original_error: String,
        /// Entry ULIDs that were successfully reversed before the failure.
        completed: Vec<String>,
        /// The entry ULID that failed.
        failed_entry: String,
        /// Whether rollback of completed entries succeeded.
        rollback_succeeded: bool,
    },

    /// Cannot restore from trash because the data file is missing.
    #[error("cannot restore from trash: data file not found at {path}")]
    RestoreFromTrashFailed { path: PathBuf },

    /// YAML serialization/deserialization error (without file path context).
    #[error("YAML error: {0}")]
    YamlSerde(#[from] serde_yaml_ng::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
