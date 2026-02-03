//! Kanban board engine with file-backed storage
//!
//! This crate provides a kanban board implementation that stores all data as JSON files
//! in a `.kanban` directory. It's designed for git-friendly task management with
//! support for concurrent access via file locking.
//!
//! ## Overview
//!
//! - **One repo = one board** - The `.kanban` directory lives at the repo root
//! - **File-per-task** - Tasks are individual JSON files for clean git diffs
//! - **Git-friendly** - Human-readable JSON, no binary formats
//! - **Agent-aware** - Tracks which agent/user modified tasks and why
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use swissarmyhammer_kanban::{KanbanContext, board::InitBoard, task::AddTask, Execute};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize a new board
//! let ctx = KanbanContext::new("/path/to/repo/.kanban");
//! InitBoard::new("My Project").execute(&ctx).await?;
//!
//! // Add a task
//! let result = AddTask::new("Implement feature X")
//!     .with_description("Add the new feature")
//!     .execute(&ctx).await?;
//!
//! println!("Created task: {}", result["id"]);
//! # Ok(())
//! # }
//! ```
//!
//! ## Storage Structure
//!
//! ```text
//! repo/
//! └── .kanban/
//!     ├── board.json         # Board metadata and column definitions
//!     ├── tasks/
//!     │   ├── {id}.json      # Current task state
//!     │   ├── {id}.jsonl     # Per-task operation log
//!     │   └── ...
//!     └── activity/
//!         ├── 000001.jsonl   # Global log (archived)
//!         └── current.jsonl  # Active global log
//! ```

mod context;
mod error;
pub mod parse;
mod processor;
pub mod types;

// Command modules
pub mod activity;
pub mod actor;
pub mod board;
pub mod column;
pub mod comment;
pub mod swimlane;
pub mod tag;
pub mod task;

// Re-export Execute trait and types from operations crate
pub use swissarmyhammer_operations::{
    async_trait, Execute, ExecutionResult, Operation, OperationProcessor,
};

pub use context::{KanbanContext, KanbanLock};
pub use error::{KanbanError, Result};
pub use processor::KanbanOperationProcessor;

// Re-export commonly used types
pub use types::{
    Actor, ActorId, Attachment, Board, Column, ColumnId, Comment, CommentId, LogEntry, Noun,
    Operation as KanbanOperation, OperationResult, Ordinal, Position, Subtask, Swimlane,
    SwimlaneId, Tag, TagId, Task, TaskId, Verb,
};
