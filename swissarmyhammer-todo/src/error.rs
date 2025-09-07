//! Error types for todo management operations

use thiserror::Error;

/// Result type for todo operations
pub type Result<T> = std::result::Result<T, TodoError>;

/// Errors that can occur during todo operations
#[derive(Debug, Error)]
pub enum TodoError {
    /// I/O operation failed
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization/deserialization failed
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Common error from swissarmyhammer-common
    #[error("Common error: {0}")]
    Common(#[from] swissarmyhammer_common::SwissArmyHammerError),

    /// Invalid todo list name
    #[error("Invalid todo list name: {0}")]
    InvalidTodoListName(String),

    /// Invalid todo item ID
    #[error("Invalid todo item ID: {0}")]
    InvalidTodoId(String),

    /// Todo list not found
    #[error("Todo list '{0}' not found")]
    TodoListNotFound(String),

    /// Todo item not found
    #[error("Todo item '{0}' not found in list '{1}'")]
    TodoItemNotFound(String, String),

    /// Empty task description
    #[error("Task description cannot be empty")]
    EmptyTask,

    /// Generic todo operation error
    #[error("Todo operation failed: {0}")]
    Other(String),
}

impl TodoError {
    /// Create a new generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}