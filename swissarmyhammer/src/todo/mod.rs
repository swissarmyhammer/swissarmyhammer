//! Todo list management system for ephemeral task tracking
//!
//! This module provides a temporary task management system that stores todo items
//! as YAML files in the local filesystem. Unlike issues, todo lists are ephemeral
//! and designed for session-based task management.
//!
//! ## Features
//!
//! - **Sequential ULID IDs**: Auto-generated sequential ULID identifiers for todo items
//! - **YAML Storage**: Human-readable YAML format for easy editing and debugging
//! - **Ephemeral**: Files stored locally and never committed to version control
//! - **Context Support**: Optional context field for implementation notes and references
//! - **FIFO Processing**: "next" pattern encourages sequential task completion
//!
//! ## Basic Usage
//!
//! ```rust
//! use swissarmyhammer::todo::{TodoStorage, TodoItem};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new todo storage
//! let storage = TodoStorage::new_default()?;
//!
//! // Create a new todo item
//! let item = storage.create_todo_item(
//!     "implement_feature",
//!     "Implement file read functionality",
//!     Some("Use existing codebase patterns for inspiration".to_string())
//! ).await?;
//! println!("Created todo item with ID: {}", item.id);
//!
//! // Get the next incomplete item
//! let next_item = storage.get_next_todo("implement_feature").await?;
//! if let Some(item) = next_item {
//!     println!("Next task: {}", item.task);
//! }
//!
//! // Mark item as complete
//! storage.mark_todo_complete("implement_feature", &item.id).await?;
//! # Ok(())
//! # }
//! ```

use crate::common::generate_monotonic_ulid;
use crate::directory_utils;
use crate::error::{Result, SwissArmyHammerError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Storage backend for todo persistence and retrieval
pub mod storage;
pub use storage::TodoStorage;

/// A unique identifier for todo items using ULID
///
/// ULIDs provide both uniqueness and natural ordering for todo items,
/// enabling sequential processing and chronological tracking.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TodoId(String);

impl TodoId {
    /// Create a new unique todo item identifier using ULID generation
    pub fn new() -> Self {
        Self(generate_monotonic_ulid().to_string())
    }

    /// Create a todo ID from a string
    pub fn from_string(id: String) -> Result<Self> {
        if id.trim().is_empty() {
            return Err(SwissArmyHammerError::Other(
                "Todo ID cannot be empty".to_string(),
            ));
        }
        Ok(Self(id))
    }

    /// Get the string representation of the todo ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TodoId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TodoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TodoId {
    type Err = SwissArmyHammerError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_string(s.to_string())
    }
}

impl AsRef<str> for TodoId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A single todo item with task description and optional context
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    /// Unique identifier for this todo item
    pub id: TodoId,
    /// Brief description of the task to be completed
    pub task: String,
    /// Optional additional context, notes, or implementation details
    pub context: Option<String>,
    /// Boolean flag indicating completion status
    pub done: bool,
}

impl TodoItem {
    /// Create a new todo item
    pub fn new(task: String, context: Option<String>) -> Self {
        Self {
            id: TodoId::new(),
            task,
            context,
            done: false,
        }
    }

    /// Mark this todo item as complete
    pub fn mark_complete(&mut self) {
        self.done = true;
    }

    /// Check if this todo item is complete
    pub fn is_complete(&self) -> bool {
        self.done
    }
}

/// A todo list containing multiple todo items
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoList {
    /// List of todo items
    pub todo: Vec<TodoItem>,
}

impl TodoList {
    /// Create a new empty todo list
    pub fn new() -> Self {
        Self { todo: Vec::new() }
    }

    /// Add a new todo item to the list
    pub fn add_item(&mut self, task: String, context: Option<String>) -> &TodoItem {
        let item = TodoItem::new(task, context);
        self.todo.push(item);
        self.todo.last().unwrap()
    }

    /// Find a todo item by ID
    pub fn find_item(&self, id: &TodoId) -> Option<&TodoItem> {
        self.todo.iter().find(|item| &item.id == id)
    }

    /// Find a mutable todo item by ID
    pub fn find_item_mut(&mut self, id: &TodoId) -> Option<&mut TodoItem> {
        self.todo.iter_mut().find(|item| &item.id == id)
    }

    /// Get the next incomplete todo item (FIFO order)
    pub fn get_next_incomplete(&self) -> Option<&TodoItem> {
        self.todo.iter().find(|item| !item.done)
    }

    /// Check if all todo items are complete
    pub fn all_complete(&self) -> bool {
        self.todo.iter().all(|item| item.done)
    }

    /// Count incomplete items
    pub fn incomplete_count(&self) -> usize {
        self.todo.iter().filter(|item| !item.done).count()
    }

    /// Count completed items
    pub fn complete_count(&self) -> usize {
        self.todo.iter().filter(|item| item.done).count()
    }
}

impl Default for TodoList {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to create a new todo item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// Brief description of the task
    pub task: String,
    /// Optional additional context or implementation notes
    pub context: Option<String>,
}

/// Request to show a todo item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// Either a specific ULID or "next" to show the next incomplete item
    pub item: String,
}

/// Request to mark a todo item as complete
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkCompleteTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// ULID of the todo item to mark as complete
    pub id: TodoId,
}

/// Determine the correct todo directory path
///
/// Returns `.swissarmyhammer/todo/` in the current repo root if in a Git repository,
/// otherwise in the current working directory.
pub fn get_todo_directory() -> Result<PathBuf> {
    let base_dir = directory_utils::find_repository_or_current_directory().map_err(|e| {
        SwissArmyHammerError::Other(format!("Failed to determine base directory: {e}"))
    })?;

    let todo_dir = base_dir.join(".swissarmyhammer").join("todo");

    // Ensure directory exists
    fs::create_dir_all(&todo_dir).map_err(|e| {
        SwissArmyHammerError::Other(format!("Failed to create todo directory: {e}"))
    })?;

    Ok(todo_dir)
}

/// Validate todo list name
pub fn validate_todo_list_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(SwissArmyHammerError::Other(
            "Todo list name cannot be empty".to_string(),
        ));
    }

    // Check for invalid characters that would cause filesystem issues
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    for ch in invalid_chars {
        if name.contains(ch) {
            return Err(SwissArmyHammerError::Other(format!(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_id_generation() {
        let id1 = TodoId::new();
        let id2 = TodoId::new();

        assert_ne!(id1, id2);
        assert_eq!(id1.as_str().len(), 26);
        assert_eq!(id2.as_str().len(), 26);
    }

    #[test]
    fn test_todo_id_from_string() {
        let id_str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let id = TodoId::from_string(id_str.to_string()).unwrap();
        assert_eq!(id.as_str(), id_str);
    }

    #[test]
    fn test_todo_item_creation() {
        let item = TodoItem::new("Test task".to_string(), Some("Test context".to_string()));

        assert_eq!(item.task, "Test task");
        assert_eq!(item.context, Some("Test context".to_string()));
        assert!(!item.done);
        assert!(!item.is_complete());
    }

    #[test]
    fn test_todo_item_mark_complete() {
        let mut item = TodoItem::new("Test task".to_string(), None);
        assert!(!item.is_complete());

        item.mark_complete();
        assert!(item.is_complete());
        assert!(item.done);
    }

    #[test]
    fn test_todo_list_operations() {
        let mut list = TodoList::new();

        let item = list.add_item("Task 1".to_string(), None);
        let item_id = item.id.clone();

        list.add_item("Task 2".to_string(), Some("Context 2".to_string()));

        assert_eq!(list.todo.len(), 2);
        assert_eq!(list.incomplete_count(), 2);
        assert_eq!(list.complete_count(), 0);
        assert!(!list.all_complete());

        // Test finding item
        let found_item = list.find_item(&item_id).unwrap();
        assert_eq!(found_item.task, "Task 1");

        // Test getting next incomplete
        let next = list.get_next_incomplete().unwrap();
        assert_eq!(next.task, "Task 1");

        // Mark first item complete
        let item_mut = list.find_item_mut(&item_id).unwrap();
        item_mut.mark_complete();

        assert_eq!(list.incomplete_count(), 1);
        assert_eq!(list.complete_count(), 1);
        assert!(!list.all_complete());

        // Next should now be Task 2
        let next = list.get_next_incomplete().unwrap();
        assert_eq!(next.task, "Task 2");
    }

    #[test]
    fn test_validate_todo_list_name() {
        assert!(validate_todo_list_name("valid_name").is_ok());
        assert!(validate_todo_list_name("").is_err());
        assert!(validate_todo_list_name("   ").is_err());
        assert!(validate_todo_list_name("invalid/name").is_err());
        assert!(validate_todo_list_name("invalid\\name").is_err());
        assert!(validate_todo_list_name("invalid:name").is_err());
    }
}
