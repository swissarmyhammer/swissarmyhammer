//! Error types for todo management operations

use swissarmyhammer_common::{ErrorSeverity, Severity};
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

impl Severity for TodoError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Filesystem failures that prevent todo operations
            TodoError::Io(_) => ErrorSeverity::Critical,

            // Error: Serialization and operation failures
            TodoError::Yaml(_) => ErrorSeverity::Error,
            TodoError::InvalidTodoListName(_) => ErrorSeverity::Error,
            TodoError::InvalidTodoId(_) => ErrorSeverity::Error,
            TodoError::TodoListNotFound(_) => ErrorSeverity::Error,
            TodoError::TodoItemNotFound(_, _) => ErrorSeverity::Error,
            TodoError::Other(_) => ErrorSeverity::Error,

            // Warning: Validation issues
            TodoError::EmptyTask => ErrorSeverity::Warning,

            // Delegate to wrapped error's severity
            TodoError::Common(err) => err.severity(),
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn test_todo_error_critical_severity() {
        // IO errors are critical
        let io_error = TodoError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(io_error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_todo_error_error_severity() {
        // Invalid todo list name
        let invalid_name = TodoError::InvalidTodoListName("invalid".to_string());
        assert_eq!(invalid_name.severity(), ErrorSeverity::Error);

        // Invalid todo ID
        let invalid_id = TodoError::InvalidTodoId("bad-id".to_string());
        assert_eq!(invalid_id.severity(), ErrorSeverity::Error);

        // Todo list not found
        let not_found = TodoError::TodoListNotFound("missing".to_string());
        assert_eq!(not_found.severity(), ErrorSeverity::Error);

        // Todo item not found
        let item_not_found = TodoError::TodoItemNotFound("item1".to_string(), "list1".to_string());
        assert_eq!(item_not_found.severity(), ErrorSeverity::Error);

        // Other error
        let other = TodoError::Other("something went wrong".to_string());
        assert_eq!(other.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_todo_error_warning_severity() {
        // Empty task is a warning
        let empty_task = TodoError::EmptyTask;
        assert_eq!(empty_task.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_todo_error_common_delegation() {
        // Test that Common errors delegate to wrapped error's severity
        let common_error = swissarmyhammer_common::SwissArmyHammerError::DirectoryCreation(
            "failed to create directory".to_string(),
        );
        let expected_severity = common_error.severity();

        let todo_error = TodoError::Common(common_error);
        assert_eq!(todo_error.severity(), expected_severity);
    }
}
