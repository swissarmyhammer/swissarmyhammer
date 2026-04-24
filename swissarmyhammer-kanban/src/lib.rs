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
pub mod dynamic_sources;
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
pub mod focus;
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

/// Returns the stacked builtin command YAML sources used by every kanban
/// consumer at startup: generic commands from `swissarmyhammer-commands`
/// first, then kanban-specific commands from this crate.
///
/// The ordering is load-bearing: later sources override earlier via the
/// partial-merge-by-id semantics in `CommandsRegistry::merge_yaml_value`.
/// User overrides (from `.kanban/commands/`) layer on top of this stack
/// at the call site — see [`default_commands_registry_with_overrides`].
///
/// This replaces inline stacking that every consumer (GUI, CLI, MCP,
/// tests) used to duplicate. The `test_support` crate used to carry a
/// copy under `composed_builtin_yaml_sources`; that helper now delegates
/// here so only one definition exists.
pub fn default_builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    let commands = swissarmyhammer_commands::builtin_yaml_sources();
    let kanban = builtin_yaml_sources();
    let mut out = Vec::with_capacity(commands.len() + kanban.len());
    out.extend(commands);
    out.extend(kanban);
    out
}

/// Build a fresh [`swissarmyhammer_commands::CommandsRegistry`] containing
/// the full default command stack (generic + kanban builtins).
///
/// Consumers that need user overrides can call
/// [`default_commands_registry_with_overrides`] or merge them manually via
/// [`swissarmyhammer_commands::CommandsRegistry::merge_yaml_sources`].
///
/// This is the self-composing factory the reviewer called for in PR #40:
/// `AppState` no longer has to know about both source stacks — it just
/// asks for the default registry and gets a ready-to-use one.
pub fn default_commands_registry() -> swissarmyhammer_commands::CommandsRegistry {
    let sources = default_builtin_yaml_sources();
    swissarmyhammer_commands::CommandsRegistry::from_yaml_sources(&sources)
}

/// Build a [`swissarmyhammer_commands::CommandsRegistry`] composed of the
/// default builtins plus user YAML overrides (typically loaded from a
/// board's `.kanban/commands/` directory via
/// [`swissarmyhammer_commands::load_yaml_dir`]).
///
/// Overrides apply last with partial-merge-by-id semantics, so they can
/// tweak specific fields on a builtin without replacing the whole
/// definition.
pub fn default_commands_registry_with_overrides(
    overrides: &[(String, String)],
) -> swissarmyhammer_commands::CommandsRegistry {
    let mut registry = default_commands_registry();
    if !overrides.is_empty() {
        let refs: Vec<(&str, &str)> = overrides
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        registry.merge_yaml_sources(&refs);
    }
    registry
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

/// Load a [`swissarmyhammer_commands::UIState`] from the per-consumer
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
/// [`UIState`]: swissarmyhammer_commands::UIState
/// [`UIState::load`]: swissarmyhammer_commands::UIState::load
pub fn default_ui_state(app_subdir: &str) -> swissarmyhammer_commands::UIState {
    swissarmyhammer_commands::UIState::load(ui_state_xdg_config_path(app_subdir))
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
