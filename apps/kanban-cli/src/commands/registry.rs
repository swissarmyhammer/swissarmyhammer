//! Kanban init/deinit profile + component registry.
//!
//! `kanban init` / `kanban deinit` install two kinds of thing:
//!
//! 1. **Profile artifacts** — the `kanban` MCP server registration and every
//!    builtin skill. These are declared once as a
//!    [`mirdan::install::Profile`] and applied by
//!    [`mirdan::install::init_profile`] / `deinit_profile`, the single
//!    data-driven installer shared across the tool CLIs and sah.
//! 2. **Genuine tool lifecycle** — the `.kanban/` git merge drivers. These are
//!    not install-of-an-agent concerns, so they stay on [`KanbanTool`]'s own
//!    `Initializable` impl, run via the [`InitRegistry`]. The tool is constructed
//!    *without* an injected MCP server, because MCP registration now flows
//!    through the profile's `mcp_server`.
//!
//! Both are declared here as one [`ToolInstall`] impl; mirdan owns the profile
//! construction and the install/uninstall sequencing shared with the other tool
//! CLIs.

use mirdan::install::Selector;
use swissarmyhammer_tools::mcp::tools::kanban::KanbanTool;

mirdan::declare_tool_install! {
    /// kanban's install identity, applied by `kanban init` / `kanban deinit`.
    ///
    /// The server name matches the binary and the identity `commands/serve.rs`
    /// advertises. Every builtin skill deploys at every scope, including
    /// `User` — a global install lands the full builtin skill set in the global
    /// store (`~/.skills` + the agent's global skill dir), so `init user` is a
    /// full configuration, and skill selection is not curated per consumer.
    /// [`KanbanTool`] is the only genuine lifecycle component, for its
    /// `.kanban/` git merge drivers; it is built *without* `with_mcp_server`
    /// because the profile owns the MCP registration.
    KanbanInstall {
        server: "kanban",
        skills: Selector::All,
        components: [KanbanTool::new()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirdan::test_support::{
        assert_tool_component_count, assert_tool_lifecycle_round_trip, assert_tool_profile,
    };
    use swissarmyhammer_common::lifecycle::InitScope;

    /// A representative slice of the builtin skill set; the deploy mechanism is
    /// identical regardless of which member we probe. `ci` is included because
    /// the old `kanban`-profile selector withheld it.
    const KANBAN_SKILLS: &[&str] = &["kanban", "implement", "ci"];

    /// The profile declares the `kanban serve` MCP server and every builtin
    /// skill, and nothing else.
    #[test]
    fn test_profile_declares_mcp_and_all_builtin_skills() {
        assert_tool_profile::<KanbanInstall>(&Selector::All);
    }

    /// Just the tool (`.kanban/` merge drivers). MCP registration and skill
    /// deployment moved to the profile installer.
    #[test]
    fn test_component_registry_holds_only_tool_lifecycle() {
        assert_tool_component_count::<KanbanInstall>(1);
    }

    /// Regression for Bug 1 — `init user` deploys the builtin skills
    /// (store + symlink) and registers the MCP server in the agent's global
    /// config, and `deinit user` takes both away again.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_round_trips_kanban_skills_and_mcp() {
        assert_tool_lifecycle_round_trip::<KanbanInstall>(InitScope::User, KANBAN_SKILLS);
    }

    /// The same round trip rooted at an explicit `<root>` for project scope.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_round_trips_kanban_skills_rooted() {
        assert_tool_lifecycle_round_trip::<KanbanInstall>(InitScope::Project, KANBAN_SKILLS);
    }
}
