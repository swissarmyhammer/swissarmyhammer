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
//! - **Agent-aware** - Every mutation goes through `StoreHandle`, which records a
//!   store-format `ChangelogEntry` against the entity for diff/undo replay
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
//!     ├── boards/
//!     │   └── board.yaml      # Board metadata (YAML)
//!     ├── tasks/
//!     │   ├── {id}.md         # Task (YAML frontmatter + markdown body)
//!     │   └── {id}.jsonl      # Per-task store-format changelog
//!     ├── tags/
//!     │   ├── {id}.yaml       # Tag state
//!     │   └── {id}.jsonl      # Per-tag store-format changelog
//!     ├── columns/
//!     │   └── {id}.yaml       # Column state
//!     ├── actors/
//!     │   └── {id}.yaml       # Actor state
//!     ├── projects/
//!     │   └── {id}.yaml       # Project state
//!     └── perspectives/
//!         └── {id}.yaml       # Perspective (saved view config)
//! ```
//!
//! Entity state files use YAML (or YAML frontmatter + markdown for tasks).
//! Changelog files are JSONL (one JSON object per line) recording store-format
//! patches that the projecting reader replays into field-level diffs.
//! JSON API responses remain unchanged — serde_json is used for all output.

pub mod auto_color;
pub mod clipboard;
pub mod command_seam;
mod context;
pub mod cross_board;
pub mod defaults;
pub mod derive_handlers;
pub mod dispatch;
pub mod dynamic_sources;
mod error;
pub mod notify_fanin;
pub mod parse;
mod processor;
pub mod tag_parser;
pub mod task_helpers;
pub mod types;

// Perspective types for saved view configurations
pub mod perspective;

// Domain command trait implementations
pub mod commands;

// Generic Command trait, registry, and dispatch context — inlined into
// this crate during the Stage 4 cut-over after the standalone
// `swissarmyhammer-commands` crate was deleted. The CommandService
// (registered from the 7 builtin command plugins at app startup) is now
// the sole source of command metadata; the YAML-driven `CommandsRegistry`
// only survives as a snapshot the app populates from
// `CommandService::list_command`.
pub mod commands_core;

// Command modules
pub mod actor;
pub mod attachment;
pub mod board;
pub mod column;
pub mod entity;
pub mod focus;
pub mod project;
pub mod schema;
pub mod scope_commands;
pub mod substrate;
pub mod tag;
pub mod task;
pub mod virtual_tags;

// Re-export Execute trait and types from operations crate
pub use swissarmyhammer_operations::{async_trait, Execute, ExecutionResult, OperationProcessor};

pub use context::{KanbanContext, KanbanLock};
pub use defaults::{
    builtin_actor_entities, builtin_view_definitions, kanban_compute_engine, KanbanLookup,
};
pub use derive_handlers::kanban_derive_registry;
pub use dynamic_sources::board_display_name;
pub use error::{KanbanError, Result};
pub use processor::KanbanOperationProcessor;
pub use substrate::wire_store_substrate;

// Re-export entity types for dynamic entity access
pub use swissarmyhammer_entity::changelog::{ChangeEntry, FieldChange};
pub use swissarmyhammer_entity::Entity;
pub use swissarmyhammer_entity::EntityContext;

/// Returns the kanban-specific builtin command YAML sources embedded at
/// compile time.
///
/// Always empty as of the Stage 4 cut-over: the 7 kanban YAMLs under
/// `builtin/commands/` were deleted because `CommandService` (the new
/// sole dispatch path, fed by the 7 builtin command plugins at app
/// startup) is the source of every command's metadata. The function is
/// retained so legacy callers — and the `compose_yaml_sources!` macro —
/// continue to compile while the `CommandsRegistry` is repopulated from
/// the `list command` MCP op at app startup.
pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    Vec::new()
}

/// File name for the UIState YAML config, used under every consumer's
/// XDG subdirectory. Private to this module: callers go through
/// [`default_ui_state`] rather than constructing paths by hand.
const UI_STATE_FILE_NAME: &str = "ui-state.yaml";

/// Resolve the per-consumer UIState config path under the XDG config
/// hierarchy: `$XDG_CONFIG_HOME/sah/<app_subdir>/ui-state.yaml`.
///
/// Falls back to `./{app_subdir}/ui-state.yaml` when the XDG base
/// directory cannot be determined (e.g. no `$HOME` in a sandboxed
/// environment). The fallback matches the legacy behavior the GUI
/// crate used to implement inline, so existing installs keep finding
/// their config even when XDG resolution fails.
pub(crate) fn ui_state_xdg_config_path(app_subdir: &str) -> std::path::PathBuf {
    use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};

    ManagedDirectory::<SwissarmyhammerConfig>::xdg_config()
        .map(|dir| dir.root().join(app_subdir).join(UI_STATE_FILE_NAME))
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(".")
                .join(app_subdir)
                .join(UI_STATE_FILE_NAME)
        })
}

/// Load a [`swissarmyhammer_ui_state::UIState`] from the per-consumer
/// XDG config file, or return defaults if the file is missing or
/// malformed.
///
/// This is the self-composing entry point consumers (GUI, CLI, MCP)
/// use at startup. It resolves
/// `$XDG_CONFIG_HOME/sah/<app_subdir>/ui-state.yaml` — keeping XDG
/// awareness out of the Tier 0 `swissarmyhammer-commands` crate — and
/// delegates the actual file I/O to [`UIState::load`], which remains
/// path-driven and consumer-agnostic.
///
/// The `app_subdir` identifies the consumer (e.g. `"kanban-app"`,
/// `"kanban-cli"`) so each one gets its own config without stepping on
/// the others. Subsequent mutations auto-save to the resolved path
/// just as with [`UIState::load`].
///
/// [`UIState`]: swissarmyhammer_ui_state::UIState
/// [`UIState::load`]: swissarmyhammer_ui_state::UIState::load
pub fn default_ui_state(app_subdir: &str) -> swissarmyhammer_ui_state::UIState {
    swissarmyhammer_ui_state::UIState::load(ui_state_xdg_config_path(app_subdir))
}

// Re-export commonly used types
pub use types::{
    default_column_entities, ActorId, ColumnId, Noun, Operation as KanbanOperation, Ordinal,
    Position, ProjectId, TagId, TaskId, Verb,
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

    /// Stage 4 of the kanban cut-over deleted the 7 builtin command YAMLs
    /// under `swissarmyhammer-kanban/builtin/commands/` because
    /// `CommandService` (fed by the 7 builtin command plugins at app
    /// startup) is now the sole source of command metadata. This test
    /// pins that decision so a future regression cannot silently re-embed
    /// YAML sources here and reintroduce the two-source confusion.
    #[test]
    fn builtin_yaml_sources_is_empty_after_cutover() {
        assert!(builtin_yaml_sources().is_empty());
    }
}
