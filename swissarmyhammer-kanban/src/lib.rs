//! Kanban board engine with file-backed storage
//!
//! This crate provides a kanban board implementation that stores all data as YAML/Markdown
//! files in a `.kanban` directory. It's designed for git-friendly task management with
//! support for concurrent access via file locking.
//!
//! ## Overview
//!
//! - **One repo = one board** - The `.kanban` directory lives at the repo root
//! - **File-per-entity** - Tasks, tags, columns, actors, swimlanes are individual files
//! - **Git-friendly** - Human-readable YAML/Markdown, no binary formats
//! - **Agent-aware** - Per-entity JSONL logs track which agent/user modified what and why
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
//!     ├── board.yaml          # Board metadata (YAML)
//!     ├── board.jsonl          # Board operation log
//!     ├── tasks/
//!     │   ├── {id}.md          # Task (YAML frontmatter + markdown body)
//!     │   ├── {id}.jsonl       # Per-task operation log
//!     ├── tags/
//!     │   ├── {id}.yaml        # Tag state
//!     │   ├── {id}.jsonl       # Per-tag operation log
//!     ├── columns/
//!     │   ├── {id}.yaml        # Column state
//!     │   ├── {id}.jsonl       # Per-column operation log
//!     ├── swimlanes/
//!     │   ├── {id}.yaml        # Swimlane state
//!     │   ├── {id}.jsonl       # Per-swimlane operation log
//!     ├── actors/
//!     │   ├── {id}.yaml        # Actor state
//!     │   ├── {id}.jsonl       # Per-actor operation log
//!     └── activity/
//!         └── current.jsonl    # Global operation log
//! ```
//!
//! Entity state files use YAML (or YAML frontmatter + markdown for tasks).
//! Operation logs use JSONL (one JSON object per line, newest first).
//! JSON API responses remain unchanged — serde_json is used for all output.

pub mod auto_color;
mod context;
pub mod defaults;
mod error;
pub mod parse;
mod processor;
pub mod tag_parser;
pub mod types;

// Command modules
pub mod activity;
pub mod actor;
pub mod attachment;
pub mod board;
pub mod column;
pub mod comment;
pub mod schema;
pub mod swimlane;
pub mod tag;
pub mod task;

// Re-export Execute trait and types from operations crate
pub use swissarmyhammer_operations::{
    async_trait, Execute, ExecutionResult, Operation, OperationProcessor,
};

pub use context::{KanbanContext, KanbanLock, MigrationStats};
pub use defaults::{kanban_defaults, KanbanLookup};
pub use error::{KanbanError, Result};
pub use processor::KanbanOperationProcessor;

// Re-export commonly used types
pub use types::{
    Actor, ActorId, Attachment, Board, Column, ColumnId, Comment, CommentId, LogEntry, Noun,
    Operation as KanbanOperation, OperationResult, Ordinal, Position, Swimlane, SwimlaneId, Tag,
    TagId, Task, TaskId, Verb,
};
