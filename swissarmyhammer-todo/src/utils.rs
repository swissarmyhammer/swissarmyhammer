//! Utility functions for todo management

use crate::error::{Result, TodoError};
use std::fs;
use std::path::PathBuf;
use swissarmyhammer_common::SwissarmyhammerDirectory;

/// Determine the correct todo directory path
///
/// Returns `.swissarmyhammer/todo/` in the Git repository root.
/// Requires being within a Git repository - no fallback to current directory.
///
/// For testing purposes, the directory can be overridden by setting the
/// `SWISSARMYHAMMER_TODO_DIR` environment variable.
pub fn get_todo_directory() -> Result<PathBuf> {
    // Check for environment variable override (useful for testing)
    if let Ok(override_dir) = std::env::var("SWISSARMYHAMMER_TODO_DIR") {
        let todo_dir = PathBuf::from(override_dir);
        // Ensure the override directory exists
        fs::create_dir_all(&todo_dir)
            .map_err(|e| TodoError::other(format!("Failed to create todo directory: {e}")))?;
        return Ok(todo_dir);
    }

    let sah_dir = SwissarmyhammerDirectory::from_git_root()
        .map_err(|e| {
            TodoError::other(format!(
                "Todo operations require a Git repository. Please run this command from within a Git repository: {}",
                e
            ))
        })?;

    let todo_dir = sah_dir.ensure_subdir("todo")
        .map_err(|e| TodoError::other(format!("Failed to create todo directory: {e}")))?;

    Ok(todo_dir)
}

/// Validate todo list name
pub fn validate_todo_list_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(TodoError::InvalidTodoListName(
            "Todo list name cannot be empty".to_string(),
        ));
    }

    // Check for invalid characters that would cause filesystem issues
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    for ch in invalid_chars {
        if name.contains(ch) {
            return Err(TodoError::InvalidTodoListName(format!(
                "Todo list name contains invalid character '{ch}': '{name}'"
            )));
        }
    }

    Ok(())
}

/// Get the path to a todo list file
pub fn get_todo_list_path(todo_list: &str) -> Result<PathBuf> {
    validate_todo_list_name(todo_list)?;
    let todo_dir = get_todo_directory()?;
    Ok(todo_dir.join(format!("{todo_list}.todo.yaml")))
}
