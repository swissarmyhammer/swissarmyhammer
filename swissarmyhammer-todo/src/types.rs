//! Core types for todo management

use crate::error::{Result, TodoError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use swissarmyhammer_common::generate_monotonic_ulid;

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
            return Err(TodoError::InvalidTodoId(
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
    type Err = TodoError;

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
    /// UTC timestamp when this todo was created
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when this todo was last updated
    #[serde(default = "default_timestamp")]
    pub updated_at: DateTime<Utc>,
}

/// Default timestamp for backward compatibility with old YAML files
fn default_timestamp() -> DateTime<Utc> {
    Utc::now()
}

impl TodoItem {
    /// Create a new todo item
    pub fn new(task: String, context: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: TodoId::new(),
            task,
            context,
            done: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Mark this todo item as complete
    pub fn mark_complete(&mut self) {
        self.done = true;
        self.updated_at = Utc::now();
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
    /// Brief description of the task (alias: title)
    #[serde(alias = "title")]
    pub task: String,
    /// Optional additional context or implementation notes (alias: description)
    #[serde(alias = "description")]
    pub context: Option<String>,
}

/// Request to show a todo item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowTodoRequest {
    /// Either a specific ULID or "next" to show the next incomplete item
    pub item: String,
}

/// Request to mark a todo item as complete
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkCompleteTodoRequest {
    /// ULID of the todo item to mark as complete
    pub id: TodoId,
}

/// Request to list todo items
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListTodosRequest {
    /// Optional filter by completion status
    /// - None: Show all todos (default)
    /// - Some(true): Show only completed todos
    /// - Some(false): Show only incomplete todos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_mark_complete_updates_timestamp() {
        // Create a new todo item
        let mut item = TodoItem::new("Test task".to_string(), None);

        // Store the original timestamps
        let original_created_at = item.created_at;
        let original_updated_at = item.updated_at;

        // Sleep briefly to ensure time has passed
        sleep(Duration::from_millis(10));

        // Mark the item as complete
        item.mark_complete();

        // Verify the item is marked as done
        assert!(item.is_complete(), "Item should be marked as complete");

        // Verify created_at is unchanged
        assert_eq!(
            item.created_at, original_created_at,
            "created_at should not change when marking complete"
        );

        // Verify updated_at has been updated
        assert!(
            item.updated_at > original_updated_at,
            "updated_at should be updated when marking complete. Original: {}, Updated: {}",
            original_updated_at,
            item.updated_at
        );
    }

    #[test]
    fn test_create_todo_request_aliases() {
        // Test that 'title' alias works for 'task'
        let json_with_title = r#"{"title": "My task", "description": "Some details"}"#;
        let request: CreateTodoRequest = serde_json::from_str(json_with_title).unwrap();
        assert_eq!(request.task, "My task");
        assert_eq!(request.context, Some("Some details".to_string()));

        // Test that original field names still work
        let json_with_task = r#"{"task": "Another task", "context": "More details"}"#;
        let request2: CreateTodoRequest = serde_json::from_str(json_with_task).unwrap();
        assert_eq!(request2.task, "Another task");
        assert_eq!(request2.context, Some("More details".to_string()));

        // Test title only (no description)
        let json_title_only = r#"{"title": "Just a title"}"#;
        let request3: CreateTodoRequest = serde_json::from_str(json_title_only).unwrap();
        assert_eq!(request3.task, "Just a title");
        assert_eq!(request3.context, None);
    }
}
