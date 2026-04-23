//! Kanban board engine with file-backed storage
//!
//! This crate provides a kanban board implementation that stores all data as YAML/Markdown
//! files in a `.kanban` directory. It's designed for git-friendly task management with
//! support for concurrent access via file locking.
//!
//! ## Overview
//!
//! - **One repo = one board** - The `.kanban` directory lives at the repo root
//! - **File-per-entity** - Tasks, tags, columns, actors, projects are individual files
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
//!     ├── actors/
//!     │   ├── {id}.yaml        # Actor state
//!     │   ├── {id}.jsonl       # Per-actor operation log
//!     └── perspectives/
//!         ├── {id}.yaml        # Perspective (saved view config)
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
pub mod actor;
pub mod attachment;
pub mod board;
pub mod column;
pub mod entity;
pub mod project;
pub mod schema;
pub mod scope_commands;
pub mod tag;
pub mod task;
pub mod virtual_tags;

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

/// Builtin command YAML files embedded at compile time, kanban-specific.
///
/// The `swissarmyhammer-commands` crate is consumer-agnostic and only ships
/// generic commands (`app`, `settings`, `entity`, `ui`, `drag`). Kanban-specific
/// commands (`task`, `column`, `tag`, `attachment`, `perspective`, `file`) live
/// under `swissarmyhammer-kanban/builtin/commands/` and are contributed to the
/// composed command registry via [`builtin_yaml_sources`]. Callers layer the
/// commands crate's builtins first, then the kanban crate's builtins, then
/// user overrides — later sources override earlier by ID with partial merge.
static BUILTIN_COMMANDS: include_dir::Dir =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/builtin/commands");

/// Returns the kanban-specific builtin command YAML sources embedded at compile time.
///
/// Enumerates every `*.yaml` file directly under `builtin/commands/` via
/// `include_dir!` — adding a new kanban-specific command file requires no Rust
/// changes. The source name is the file stem (e.g. `task.yaml` → `"task"`).
///
/// The loader enforces a flat layout: only files whose parent path is the
/// root of the embedded directory are returned. `include_dir!` walks
/// recursively, but keys here are basenames only, so a nested
/// `commands/sub/foo.yaml` would silently shadow `commands/foo.yaml` on
/// `HashMap` insert downstream. Filtering to the root prevents that
/// class of bug at the loader.
pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    BUILTIN_COMMANDS
        .files()
        .filter(|file| file.path().extension().and_then(|e| e.to_str()) == Some("yaml"))
        .filter(|file| file.path().parent() == Some(std::path::Path::new("")))
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

// Re-export commonly used types
pub use types::{
    default_column_entities, ActorId, ColumnId, LogEntry, Noun, Operation as KanbanOperation,
    OperationResult, Ordinal, Position, ProjectId, TagId, TaskId, Verb,
};

/// Test-only helpers shared between this crate's unit tests and integration
/// tests, as well as downstream crates running their own integration tests.
///
/// Gated on `#[cfg(any(test, feature = "test-support"))]` so release builds
/// do not pay for these helpers. Integration tests under `tests/` see the
/// module because the crate's own `dev-dependencies` enable the
/// `test-support` feature.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
mod builtin_commands_tests {
    use super::builtin_yaml_sources;

    /// With no YAML files in `builtin/commands/`, the function returns an
    /// empty vec. This sanity-checks the `include_dir!` + flat-layout filter
    /// before any kanban-specific YAML has been moved in.
    ///
    /// Once YAML files land under `swissarmyhammer-kanban/builtin/commands/`,
    /// this test will need to be updated — the richer contents-coverage test
    /// lives in `tests/builtin_commands.rs`.
    #[test]
    fn builtin_yaml_sources_has_kanban_specific_files() {
        let sources = builtin_yaml_sources();
        // After the move, the six kanban-specific YAML files live here.
        let names: Vec<&str> = sources.iter().map(|(n, _)| *n).collect();
        for expected in ["task", "column", "tag", "attachment", "perspective", "file"] {
            assert!(
                names.contains(&expected),
                "kanban builtin commands missing `{expected}.yaml`: {names:?}",
            );
        }
    }
}
