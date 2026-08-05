//! Set up sah for all detected AI coding agents (skills + agents + MCP +
//! statusline).
//!
//! The MCP server, builtin skills, builtin agents, and statusline are all
//! installed through sah's declarative [`Profile`] via
//! [`mirdan::install::init_profile`] — sah is "just a bigger profile," not a
//! special case. The two install concerns that are not expressible as profile
//! data — the `.sah/` + `.prompts/` workspace structure and the `.kanban/`
//! merge drivers — run as the `Initializable` components registered by
//! [`crate::commands::registry::register_all`].

use crate::cli::InstallTarget;

use super::{run_lifecycle, Direction};

/// Install sah for all detected AI coding agents.
///
/// Runs sah's [`Profile`] through [`mirdan::install::init_profile`] (MCP,
/// skills, agents, statusline) and then the non-profile
/// `Initializable` components (project workspace, kanban merge drivers) in
/// priority order. Components that are not applicable to the given scope are
/// automatically skipped.
///
/// Install keeps `.sah/` and `.prompts/` in place, so the shared runner takes
/// `remove_directory: false`; only uninstall has anything to remove.
pub fn install(target: InstallTarget) -> Result<(), String> {
    run_lifecycle(Direction::Install, target, false)
}
