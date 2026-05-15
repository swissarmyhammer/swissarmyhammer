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
//! - `skill`: Resolve, render, and deploy the builtin `kanban` skill to
//!   detected agents (`KanbanSkillDeployment` — `Initializable` priority 20).
//! - `registry`: `Initializable` component registry for `kanban init` /
//!   `kanban deinit`. Exposes `register_all` and `KanbanMcpRegistration`.

pub mod doctor;
pub mod registry;
pub mod serve;
pub mod skill;
