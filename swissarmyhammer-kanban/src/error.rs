//! Error types for the kanban engine

use std::path::PathBuf;
use thiserror::Error;

/// Result type for kanban operations
pub type Result<T> = std::result::Result<T, KanbanError>;

/// Errors that can occur in kanban operations
#[derive(Debug, Error)]
pub enum KanbanError {
    /// Board not initialized at the given path
    #[error("board not initialized at {path}")]
    NotInitialized { path: PathBuf },

    /// Board already exists
    #[error("board already exists at {path}")]
    AlreadyExists { path: PathBuf },

    /// Task not found
    #[error("task not found: {id}")]
    TaskNotFound { id: String },

    /// Column not found
    #[error("column not found: {id}")]
    ColumnNotFound { id: String },

    /// Swimlane not found
    #[error("swimlane not found: {id}")]
    SwimlaneNotFound { id: String },

    /// Actor not found
    #[error("actor not found: {id}")]
    ActorNotFound { id: String },

    /// Tag not found
    #[error("tag not found: {id}")]
    TagNotFound { id: String },

    /// Comment not found
    #[error("comment not found: {id}")]
    CommentNotFound { id: String },

    /// Generic resource not found (for subtasks, attachments, etc.)
    #[error("{resource} not found: {id}")]
    NotFound { resource: String, id: String },

    /// Column has tasks and cannot be deleted
    #[error("column '{id}' has {count} tasks and cannot be deleted")]
    ColumnNotEmpty { id: String, count: usize },

    /// Swimlane has tasks and cannot be deleted
    #[error("swimlane '{id}' has {count} tasks and cannot be deleted")]
    SwimlaneNotEmpty { id: String, count: usize },

    /// Duplicate ID
    #[error("duplicate {item_type} ID: {id}")]
    DuplicateId { item_type: String, id: String },

    /// Dependency cycle detected
    #[error("dependency cycle detected: {path}")]
    DependencyCycle { path: String },

    /// Invalid operation
    #[error("invalid operation: {verb} {noun}")]
    InvalidOperation { verb: String, noun: String },

    /// Parse error
    #[error("parse error: {message}")]
    Parse { message: String },

    /// Missing required field
    #[error("missing required field: {field}")]
    MissingField { field: String },

    /// Invalid field value
    #[error("invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    /// Lock is held by another process
    #[error("lock busy - another operation in progress")]
    LockBusy,

    /// Lock timeout
    #[error("lock timeout after {elapsed_ms}ms")]
    LockTimeout { elapsed_ms: u64 },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl KanbanError {
    /// Create a parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    /// Create a missing field error
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Create an invalid value error
    pub fn invalid_value(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidValue {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a duplicate ID error
    pub fn duplicate_id(item_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self::DuplicateId {
            item_type: item_type.into(),
            id: id.into(),
        }
    }

    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::LockBusy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = KanbanError::TaskNotFound {
            id: "abc123".into(),
        };
        assert_eq!(err.to_string(), "task not found: abc123");
    }

    #[test]
    fn test_parse_error() {
        let err = KanbanError::parse("unexpected token");
        assert!(err.to_string().contains("unexpected token"));
    }

    #[test]
    fn test_retryable() {
        assert!(KanbanError::LockBusy.is_retryable());
        assert!(!KanbanError::TaskNotFound { id: "x".into() }.is_retryable());
    }
}
