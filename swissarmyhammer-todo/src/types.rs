//! Core types for todo management

use crate::error::{Result, TodoError};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use swissarmyhammer_common::generate_monotonic_ulid;

/// Plan entry status lifecycle
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanEntryStatus {
    /// Entry is pending execution
    #[serde(rename = "pending")]
    Pending,
    /// Entry is currently being executed
    #[serde(rename = "in_progress")]
    InProgress,
    /// Entry has been completed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Entry execution failed
    #[serde(rename = "failed")]
    Failed,
    /// Entry was cancelled before completion
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Priority levels for plan entries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// High priority - critical for task completion
    #[serde(rename = "high")]
    High,
    /// Medium priority - important but not critical
    #[serde(rename = "medium")]
    Medium,
    /// Low priority - nice to have or cleanup tasks
    #[serde(rename = "low")]
    Low,
}

/// Individual plan entry representing a specific action or step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanEntry {
    /// Unique identifier for this plan entry
    pub id: String,
    /// Human-readable description of what this entry will accomplish
    pub content: String,
    /// Priority level for execution order and importance
    pub priority: Priority,
    /// Current execution status
    pub status: PlanEntryStatus,
    /// Optional additional context or notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Timestamp when this entry was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<SystemTime>,
    /// Timestamp when this entry was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<SystemTime>,
}

impl PlanEntry {
    /// Create a new plan entry with pending status
    pub fn new(content: String, priority: Priority) -> Self {
        let now = SystemTime::now();
        Self {
            id: generate_monotonic_ulid().to_string(),
            content,
            priority,
            status: PlanEntryStatus::Pending,
            notes: None,
            created_at: Some(now),
            updated_at: Some(now),
        }
    }

    /// Update the status of this plan entry
    pub fn update_status(&mut self, new_status: PlanEntryStatus) {
        if self.status != new_status {
            self.status = new_status;
            self.updated_at = Some(SystemTime::now());
        }
    }

    /// Add or update notes for this plan entry
    pub fn set_notes(&mut self, notes: String) {
        self.notes = Some(notes);
        self.updated_at = Some(SystemTime::now());
    }

    /// Check if this plan entry is complete (completed, failed, or cancelled)
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            PlanEntryStatus::Completed | PlanEntryStatus::Failed | PlanEntryStatus::Cancelled
        )
    }

    /// Check if this plan entry is currently being executed
    pub fn is_in_progress(&self) -> bool {
        matches!(self.status, PlanEntryStatus::InProgress)
    }
}

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

/// Type alias for TodoItem - now using PlanEntry from claude-agent
pub type TodoItem = PlanEntry;

/// Extension trait for PlanEntry to provide TodoItem-compatible methods
pub trait TodoItemExt {
    /// Create a new todo item with default priority
    fn new_todo(task: String, context: Option<String>) -> Self;

    /// Get the task description (content)
    fn task(&self) -> &str;

    /// Get the context (notes)
    fn context(&self) -> Option<&String>;

    /// Check if done (status is Completed, Failed, or Cancelled)
    fn done(&self) -> bool;

    /// Mark this todo item as complete
    fn mark_complete(&mut self);
}

impl TodoItemExt for PlanEntry {
    fn new_todo(task: String, context: Option<String>) -> Self {
        let mut entry = PlanEntry::new(task, Priority::Medium);
        if let Some(ctx) = context {
            entry.set_notes(ctx);
        }
        entry
    }

    fn task(&self) -> &str {
        &self.content
    }

    fn context(&self) -> Option<&String> {
        self.notes.as_ref()
    }

    fn done(&self) -> bool {
        self.is_complete()
    }

    fn mark_complete(&mut self) {
        self.update_status(PlanEntryStatus::Completed);
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
        let item = TodoItem::new_todo(task, context);
        self.todo.push(item);
        self.todo.last().unwrap()
    }

    /// Find a todo item by ID
    pub fn find_item(&self, id: &TodoId) -> Option<&TodoItem> {
        self.todo.iter().find(|item| item.id == id.as_str())
    }

    /// Find a mutable todo item by ID
    pub fn find_item_mut(&mut self, id: &TodoId) -> Option<&mut TodoItem> {
        self.todo.iter_mut().find(|item| item.id == id.as_str())
    }

    /// Get the next incomplete todo item (FIFO order)
    pub fn get_next_incomplete(&self) -> Option<&TodoItem> {
        self.todo.iter().find(|item| !item.done())
    }

    /// Check if all todo items are complete
    pub fn all_complete(&self) -> bool {
        self.todo.iter().all(|item| item.done())
    }

    /// Count incomplete items
    pub fn incomplete_count(&self) -> usize {
        self.todo.iter().filter(|item| !item.done()).count()
    }

    /// Count completed items
    pub fn complete_count(&self) -> usize {
        self.todo.iter().filter(|item| item.done()).count()
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
    /// Brief description of the task
    pub task: String,
    /// Optional additional context or implementation notes
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
        let mut item = TodoItem::new_todo("Test task".to_string(), None);

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
            "updated_at should be updated when marking complete. Original: {:?}, Updated: {:?}",
            original_updated_at,
            item.updated_at
        );
    }
}
