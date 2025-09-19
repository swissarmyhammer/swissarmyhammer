//! Todo list management system for ephemeral task tracking
//!
//! This crate provides a temporary task management system that stores todo items
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
//! use swissarmyhammer_todo::{TodoStorage, TodoItem};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new todo storage
//! let storage = TodoStorage::new_default()?;
//!
//! // Create a new todo item
//! let item = storage.create_todo_item(
//!     "Implement file read functionality".to_string(),
//!     Some("Use existing codebase patterns for inspiration".to_string())
//! ).await?;
//! println!("Created todo item with ID: {}", item.id);
//!
//! // Get the next incomplete item
//! let next_item = storage.get_todo_item("next").await?;
//! if let Some(item) = next_item {
//!     println!("Next task: {}", item.task);
//! }
//!
//! // Mark item as complete
//! storage.mark_todo_complete(&item.id).await?;
//! # Ok(())
//! # }
//! ```

mod error;
mod storage;
mod types;
mod utils;

// Re-exports
pub use error::{Result, TodoError};
pub use storage::TodoStorage;
pub use types::{
    CreateTodoRequest, MarkCompleteTodoRequest, ShowTodoRequest, TodoId, TodoItem, TodoList,
};
pub use utils::{get_todo_directory, get_todo_list_path, validate_todo_list_name};
