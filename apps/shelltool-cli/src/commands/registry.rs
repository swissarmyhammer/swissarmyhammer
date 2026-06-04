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

use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// The MCP server name registered under each agent's config.
const SERVER_NAME: &str = "shelltool";

/// The builtin skill shelltool deploys.
const SKILL_NAME: &str = "shell";

/// The declarative manifest of what `shelltool init`/`deinit` install through
/// mirdan's profile installer: the `shelltool serve` MCP server and the single
/// builtin `shell` skill. No agents.
///
/// Skills deploy at every scope, including `User` — a global install lands the
/// `shell` skill in the global store (`~/.skills` + the agent's global skill
/// dir), so `init user` is a full configuration. This matches sah's
/// `Selector::All`, which already deploys at every scope. The `scope` parameter
/// is retained for signature parity with the other consumers (and forwarded to
/// the installer by the caller), but no longer gates skill selection.
pub fn profile(_scope: InitScope) -> mirdan::install::Profile {
    mirdan::install::Profile {
        mcp_server: Some(mirdan::install::ProfileMcpServer::serve(SERVER_NAME)),
        skills: Some(mirdan::install::Selector::Single(SKILL_NAME.to_string())),
        agents: None,
        statusline: false,
        preamble: false,
    }
}

/// Register the genuine tool-lifecycle components into `registry`.
///
/// Only [`ShellExecuteTool`] is registered — for its `Bash` denial and
/// `.shell/config.yaml` lifecycle. It is built *without* `with_mcp_server`: the
/// `shelltool` MCP registration is owned by the profile (see [`profile`]), not
/// the tool.
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(ShellExecuteTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirdan::install::init_profile;
    use mirdan::test_support::{
        assert_no_init_error, write_single_agent_config, MirdanConfigGuard, ProjectScopeDeploy,
        UserScopeDeploy,
    };
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    #[test]
    fn test_profile_declares_mcp_and_shell_skill_in_project_scope() {
        let profile = profile(InitScope::Project);
        let server = profile.mcp_server.expect("profile declares an MCP server");
        assert_eq!(server.name, "shelltool");
        assert_eq!(server.command, "shelltool");
        assert_eq!(server.args, vec!["serve".to_string()]);
        assert_eq!(
            profile.skills,
            Some(mirdan::install::Selector::Single("shell".to_string()))
        );
        assert!(profile.agents.is_none());
        assert!(!profile.statusline);
        assert!(!profile.preamble);
    }

    #[test]
    fn test_user_scope_selects_skills() {
        // Regression: `init user` must deploy skills too (into the global store),
        // not just register the MCP server. The User-scope gate that returned
        // `None` here was the bug.
        let profile = profile(InitScope::User);
        assert!(profile.mcp_server.is_some());
        assert_eq!(
            profile.skills,
            Some(mirdan::install::Selector::Single("shell".to_string())),
            "user scope must select the shell skill"
        );
    }

    #[test]
    fn test_local_scope_deploys_skills() {
        assert!(profile(InitScope::Local).skills.is_some());
    }

    #[test]
    fn test_register_all_registers_only_tool_lifecycle() {
        // Just the tool (Bash deny + `.shell/config.yaml`). MCP registration and
        // skill deployment moved to the profile installer.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 1);
    }

    /// Regression for Bug 1 — `init user` deploys the `shell` skill (store +
    /// symlink) and registers the MCP server in the agent's global config.
    ///
    /// Drives the REAL `profile(InitScope::User)` through `mirdan::install::
    /// init_profile`, so a missing skill in the production profile fails here
    /// (no reconstruction to mirror the bug).
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_deploys_shell_skill_and_registers_mcp() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let work = env.temp_dir().canonicalize().unwrap();
        let _cwd = CurrentDirGuard::new(&work).unwrap();
        let config_path = write_single_agent_config(&work, &env.home_path());
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results = init_profile(
            &profile(InitScope::User),
            InitScope::User,
            None,
            &NullReporter,
        );
        assert_no_init_error("shelltool user init", &results);

        UserScopeDeploy {
            home: &env.home_path(),
            server: "shelltool",
            skills: &["shell"],
        }
        .assert();
    }

    /// Project-scope deploy rooted at an explicit `<root>` — skills land in
    /// `<root>/.skills/` + the agent's project skill dir, MCP in the project
    /// config. No CWD access.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_deploys_shell_skill_rooted() {
        let env = IsolatedTestEnvironment::new().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().canonicalize().unwrap();
        let config_path = write_single_agent_config(&root, &env.home_path());
        let _mirdan = MirdanConfigGuard::set(&config_path);

        let results = init_profile(
            &profile(InitScope::Project),
            InitScope::Project,
            Some(&root),
            &NullReporter,
        );
        assert_no_init_error("shelltool project init", &results);

        ProjectScopeDeploy {
            root: &root,
            server: "shelltool",
            skills: &["shell"],
        }
        .assert();
    }
}
