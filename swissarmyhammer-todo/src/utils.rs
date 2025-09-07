//! Utility functions for todo management

use crate::error::{Result, TodoError};
use std::fs;
use std::path::PathBuf;
use swissarmyhammer_common::get_or_create_swissarmyhammer_directory;

/// Determine the correct todo directory path
///
/// Returns `.swissarmyhammer/todo/` in the Git repository root.
/// Requires being within a Git repository - no fallback to current directory.
pub fn get_todo_directory() -> Result<PathBuf> {
    let swissarmyhammer_dir = get_or_create_swissarmyhammer_directory()
        .map_err(|e| {
            TodoError::other(format!(
                "Todo operations require a Git repository. Please run this command from within a Git repository: {}",
                e
            ))
        })?;

    let todo_dir = swissarmyhammer_dir.join("todo");

    // Ensure todo subdirectory exists
    fs::create_dir_all(&todo_dir)
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
