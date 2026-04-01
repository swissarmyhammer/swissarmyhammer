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
//! InitBoard::new("My Project").execute(&ctx).await.into_result()?;
//!
//! // Add a task
//! let result = AddTask::new("Implement feature X")
//!     .with_description("Add the new feature")
//!     .execute(&ctx).await.into_result()?;
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
pub mod clipboard;
mod context;
pub mod cross_board;
pub mod defaults;
pub mod derive_handlers;
pub mod dispatch;
mod error;
pub mod parse;
mod processor;
pub mod tag_parser;
pub mod task_helpers;
pub mod types;

// Perspective types for saved view configurations
pub mod perspective;

// Domain command trait implementations
pub mod commands;

// Command modules
pub mod activity;
pub mod actor;
pub mod attachment;
pub mod board;
pub mod column;
pub mod entity;
pub mod schema;
pub mod scope_commands;
pub mod swimlane;
pub mod tag;
pub mod task;

// Re-export Execute trait and types from operations crate
pub use swissarmyhammer_operations::{
    async_trait, Execute, ExecutionResult, Operation, OperationProcessor,
};

pub use context::{KanbanContext, KanbanLock};
pub use defaults::{
    builtin_actor_entities, builtin_view_definitions, kanban_compute_engine, KanbanLookup,
};
pub use derive_handlers::kanban_derive_registry;
pub use error::{KanbanError, Result};
pub use processor::KanbanOperationProcessor;

// Re-export entity types for dynamic entity access
pub use swissarmyhammer_entity::changelog::{ChangeEntry, FieldChange};
pub use swissarmyhammer_entity::Entity;
pub use swissarmyhammer_entity::EntityContext;

// Re-export commonly used types
pub use types::{
    default_column_entities, ActorId, ColumnId, LogEntry, Noun, Operation as KanbanOperation,
    OperationResult, Ordinal, Position, SwimlaneId, TagId, TaskId, Verb,
};
