//! Shelltool init/deinit profile + component registry.
//!
//! `shelltool init` / `shelltool deinit` install two kinds of thing:
//!
//! 1. **Profile artifacts** — the `shelltool` MCP server registration and the
//!    builtin `shell` skill. These are declared once as a [`mirdan::install::Profile`]
//!    and applied by [`mirdan::install::init_profile`] / `deinit_profile`, the
//!    single data-driven installer shared across the tool CLIs and sah.
//! 2. **Genuine tool lifecycle** — the `Bash` tool denial and the
//!    `.shell/config.yaml` template. These are not install-of-an-agent concerns,
//!    so they stay on [`ShellExecuteTool`]'s own `Initializable` impl, run via the
//!    [`InitRegistry`]. The tool is constructed *without* an injected MCP server,
//!    because MCP registration now flows through the profile's `mcp_server`.
//!
//! Both are declared here as one [`ToolInstall`] impl; mirdan owns the profile
//! construction and the install/uninstall sequencing shared with the other tool
//! CLIs.
//!
//! The impl is written out rather than declared through
//! `mirdan::declare_tool_install!`. That macro takes each component as one
//! expression of the component's own type, and [`ShellExecuteTool::new`]
//! answers a `Result`, because the shell state it opens reads the filesystem.

use mirdan::install::Selector;
use mirdan::tool_install::ToolInstall;
use swissarmyhammer_common::lifecycle::InitRegistry;
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// The builtin skill shelltool deploys.
const SKILL_NAME: &str = "shell";

/// shelltool's install identity, applied by `shelltool init` /
/// `shelltool deinit`.
///
/// The server name matches the binary and the identity `commands/serve.rs`
/// advertises. The single builtin `shell` skill deploys at every scope,
/// including `User` — a global install lands it in the global store
/// (`~/.skills` + the agent's global skill dir), so `init user` is a full
/// configuration. [`ShellExecuteTool`] is the only genuine lifecycle
/// component, for its `Bash` denial and `.shell/config.yaml`; it is built
/// *without* `with_mcp_server` because the profile owns the MCP
/// registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShelltoolInstall;

impl ToolInstall for ShelltoolInstall {
    const SERVER_NAME: &'static str = "shelltool";

    fn skills() -> Selector {
        Selector::Single(SKILL_NAME.to_string())
    }

    /// Registers the one lifecycle component, [`ShellExecuteTool`].
    ///
    /// A tool whose shell state cannot be created is reported through
    /// `tracing::error!` and left out, so an install that can still write the
    /// profile writes it.
    fn register_components(registry: &mut InitRegistry) {
        match ShellExecuteTool::new() {
            Ok(tool) => registry.register(tool),
            Err(error) => {
                tracing::error!(%error, "shell tool lifecycle component not registered")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirdan::test_support::{
        assert_tool_component_count, assert_tool_lifecycle_round_trip, assert_tool_profile,
    };
    use swissarmyhammer_common::lifecycle::InitScope;

    /// The one builtin skill shelltool must deploy.
    const SHELLTOOL_SKILLS: &[&str] = &[SKILL_NAME];

    /// The profile declares the `shelltool serve` MCP server and the single
    /// builtin `shell` skill, and nothing else.
    #[test]
    fn test_profile_declares_mcp_and_shell_skill() {
        assert_tool_profile::<ShelltoolInstall>(&Selector::Single(SKILL_NAME.to_string()));
    }

    /// Just the tool (Bash deny + `.shell/config.yaml`). MCP registration and
    /// skill deployment moved to the profile installer.
    #[test]
    fn test_component_registry_holds_only_tool_lifecycle() {
        assert_tool_component_count::<ShelltoolInstall>(1);
    }

    /// Regression for Bug 1 — `init user` deploys the `shell` skill (store +
    /// symlink) and registers the MCP server in the agent's global config, and
    /// `deinit user` takes both away again. The User-scope gate that returned
    /// `None` here was the bug.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_round_trips_shell_skill_and_mcp() {
        assert_tool_lifecycle_round_trip::<ShelltoolInstall>(InitScope::User, SHELLTOOL_SKILLS);
    }

    /// The same round trip rooted at an explicit `<root>` for project scope.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_round_trips_shell_skill_rooted() {
        assert_tool_lifecycle_round_trip::<ShelltoolInstall>(InitScope::Project, SHELLTOOL_SKILLS);
    }
}
