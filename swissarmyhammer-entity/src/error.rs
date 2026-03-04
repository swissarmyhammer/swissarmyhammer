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
        source: serde_yaml::Error,
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

    /// A changelog ULID was not found in the index.
    #[error("changelog entry not found: {ulid}")]
    ChangelogEntryNotFound { ulid: String },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
