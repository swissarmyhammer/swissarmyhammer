//! Root-explicit SwissArmyHammer workspace initialization.
//!
//! This crate houses the `sah init` workspace-setup logic as a lightweight,
//! reusable library. It provides composable [`Initializable`] components that
//! are **rooted at an explicit `&Path`** rather than relying on the process
//! working directory or git-root detection.
//!
//! The components here are the workspace-structure and builtin-skills slice of
//! `sah init` — the parts that make a folder a usable SwissArmyHammer
//! workspace. They deliberately exclude the Claude-Code `.claude/settings.json`
//! integration (`deny-bash`, `statusline`) and the agent-detection-based MCP /
//! subagent registration, which are concerns of the external `claude` CLI and
//! not of an in-process agent operating on a single workspace directory.
//!
//! ## Why root-explicit
//!
//! The original `sah init` components read `std::env::current_dir()` and walked
//! up to a git root. That is unsafe for a long-running multi-board desktop app:
//! mutating the process CWD to "root" an init run races every other thread.
//! Every component in this crate instead takes the workspace root as a
//! parameter, so `sah init` for an arbitrary directory never touches global
//! process state.
//!
//! ## Usage
//!
//! ```no_run
//! use std::path::Path;
//! use swissarmyhammer_common::lifecycle::InitScope;
//! use swissarmyhammer_common::reporter::NullReporter;
//! use swissarmyhammer_workspace_init::run_workspace_init;
//!
//! // Make `/some/board` a SwissArmyHammer workspace. Idempotent.
//! let results = run_workspace_init(Path::new("/some/board"), &InitScope::Project, &NullReporter);
//! assert!(results.iter().all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
//! ```

mod components;
mod registry;

pub use components::{ProjectStructure, SkillDeployment};
pub use registry::{register_workspace_init, run_workspace_init};

// Re-export the lifecycle vocabulary so consumers don't need a direct
// dependency on `swissarmyhammer-common` just to inspect init results.
pub use swissarmyhammer_common::lifecycle::{
    InitRegistry, InitResult, InitScope, InitStatus, Initializable,
};
