//! Command modules for kanban CLI.
//!
//! Each top-level subcommand implementation lives in its own module here,
//! keeping `main.rs` limited to CLI plumbing. Infrastructure files
//! (`cli.rs`, `cli_gen.rs`, `banner.rs`, `logging.rs`, `merge.rs`) remain
//! at the crate root.
//!
//! Current modules:
//! - `serve`: MCP server over stdio exposing the `kanban` operation tool.
//! - `doctor`: Diagnostic checks (`kanban doctor`) — git repo, binary on
//!   PATH, and `.kanban/board.yaml` initialization.
//! - `registry`: Profile + `Initializable` component registry for `kanban init`
//!   / `kanban deinit`. Exposes `profile` (the `kanban` MCP server + skills
//!   manifest applied by mirdan's profile installer) and `register_all` (the
//!   `KanbanTool` `.kanban/` merge-driver lifecycle).

pub mod doctor;
pub mod registry;
pub mod serve;
