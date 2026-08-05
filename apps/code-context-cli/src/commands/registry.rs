//! Code-context init/deinit profile + component registry.
//!
//! `code-context init` / `code-context deinit` install two kinds of thing:
//!
//! 1. **Profile artifacts** — the `code-context` MCP server registration and
//!    every builtin skill. These are declared once as a
//!    [`mirdan::install::Profile`] and applied by
//!    [`mirdan::install::init_profile`] / `deinit_profile`, the single
//!    data-driven installer shared across the tool CLIs and sah. Routing MCP
//!    registration through the profile's strategy-aware applier also fixes the
//!    Claude local-scope (`InitScope::Local`) handling the old hand-rolled
//!    per-agent loop silently dropped.
//! 2. **Genuine tool lifecycle** — the `.code-context/` directory and its
//!    `.gitignore` entry. These are not install-of-an-agent concerns, so they
//!    stay on [`CodeContextTool`]'s own `Initializable` impl, run via the
//!    [`InitRegistry`].
//!
//! Both are declared here as one [`ToolInstall`] impl; mirdan owns the profile
//! construction and the install/uninstall sequencing shared with the other tool
//! CLIs.

use mirdan::install::Selector;
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;

mirdan::declare_tool_install! {
    /// code-context's install identity, applied by `code-context init` /
    /// `code-context deinit`.
    ///
    /// The server name matches the binary and the identity `commands/serve.rs`
    /// advertises. Every builtin skill deploys at every scope, including
    /// `User` — a global install lands every builtin skill in the global store
    /// (`~/.skills` + the agent's global skill dir), so `init user` is a full
    /// configuration, and skill selection is not curated per consumer. The
    /// `code-context skill` subcommand reuses [`mirdan::tool_install::ToolInstall::skills`] here so
    /// the skills-only install and the full install never diverge.
    /// [`CodeContextTool`] is the only genuine lifecycle component, for its
    /// `.code-context/` directory and `.gitignore` entry.
    CodeContextInstall {
        server: "code-context",
        skills: Selector::All,
        components: [CodeContextTool::new()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirdan::test_support::{
        assert_tool_component_count, assert_tool_lifecycle_round_trip, assert_tool_profile,
    };
    use swissarmyhammer_common::lifecycle::InitScope;

    /// A representative slice of the builtin skill set code-context deploys at
    /// every scope. `ci` is included because the old four-name selector
    /// withheld it.
    const PROBE_SKILLS: &[&str] = &["code-context", "explore", "lsp", "detected-projects", "ci"];

    /// The profile declares the `code-context serve` MCP server and every
    /// builtin skill, and nothing else — curation by name or profile is gone.
    #[test]
    fn test_profile_declares_mcp_and_every_builtin_skill() {
        assert_tool_profile::<CodeContextInstall>(&Selector::All);
    }

    /// Just the tool (`.code-context/` directory). MCP registration moved to
    /// the profile installer.
    #[test]
    fn test_component_registry_holds_only_tool_lifecycle() {
        assert_tool_component_count::<CodeContextInstall>(1);
    }

    /// Regression for Bugs 1 + 2 — `init user` deploys the builtin skills
    /// (store + symlink) and registers the MCP server in the agent's global
    /// config, and `deinit user` takes both away again.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_round_trips_skills_and_mcp() {
        assert_tool_lifecycle_round_trip::<CodeContextInstall>(InitScope::User, PROBE_SKILLS);
    }

    /// The same round trip rooted at an explicit `<root>` for project scope.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_round_trips_skills_rooted() {
        assert_tool_lifecycle_round_trip::<CodeContextInstall>(InitScope::Project, PROBE_SKILLS);
    }
}
