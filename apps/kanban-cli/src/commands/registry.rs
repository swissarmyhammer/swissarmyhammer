//! Kanban init/deinit profile + component registry.
//!
//! `kanban init` / `kanban deinit` install two kinds of thing:
//!
//! 1. **Profile artifacts** — the `kanban` MCP server registration and the
//!    `kanban`-profile builtin skills (the workflow cluster: `kanban`, `plan`,
//!    `task`, `finish`, `implement`, `review`). These are declared once as a
//!    [`mirdan::install::Profile`] and applied by
//!    [`mirdan::install::init_profile`] / `deinit_profile`, the single
//!    data-driven installer shared across the tool CLIs and sah.
//! 2. **Genuine tool lifecycle** — the `.kanban/` git merge drivers. These are
//!    not install-of-an-agent concerns, so they stay on [`KanbanTool`]'s own
//!    `Initializable` impl, run via the [`InitRegistry`]. The tool is constructed
//!    *without* an injected MCP server, because MCP registration now flows
//!    through the profile's `mcp_server`.

use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_tools::mcp::tools::kanban::KanbanTool;

/// The MCP server name registered under each agent's config. Matches the binary
/// and the server identity advertised by `commands/serve.rs`.
const SERVER_NAME: &str = "kanban";

/// The init profile whose tagged builtin skills kanban deploys.
const KANBAN_PROFILE: &str = "kanban";

/// The declarative manifest of what `kanban init`/`deinit` install through
/// mirdan's profile installer: the `kanban serve` MCP server and every builtin
/// skill tagged with the `kanban` profile. No agents.
///
/// Skills deploy at every scope, including `User` — a global install lands the
/// `kanban`-profile skill cluster in the global store (`~/.skills` + the agent's
/// global skill dir), so `init user` is a full configuration. This matches sah's
/// `Selector::All`, which already deploys at every scope. The `scope` parameter
/// is retained for signature parity with the other consumers (and forwarded to
/// the installer by the caller), but no longer gates skill selection.
pub fn profile(_scope: InitScope) -> mirdan::install::Profile {
    mirdan::install::Profile {
        mcp_server: Some(mirdan::install::ProfileMcpServer::serve(SERVER_NAME)),
        skills: Some(mirdan::install::Selector::Profile(
            KANBAN_PROFILE.to_string(),
        )),
        agents: None,
        statusline: false,
        preamble: false,
    }
}

/// Register the genuine tool-lifecycle components into `registry`.
///
/// Only [`KanbanTool`] is registered — for its `.kanban/` git merge-driver
/// lifecycle. It is built *without* `with_mcp_server`: the `kanban` MCP
/// registration is owned by the profile (see [`profile`]), not the tool.
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanTool::new());
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

    /// A representative slice of the `kanban`-profile skill cluster; the deploy
    /// mechanism is identical regardless of which member we probe.
    const KANBAN_SKILLS: &[&str] = &["kanban", "implement"];

    #[test]
    fn test_profile_declares_mcp_and_kanban_profile_skills_in_project_scope() {
        let profile = profile(InitScope::Project);
        let server = profile.mcp_server.expect("profile declares an MCP server");
        assert_eq!(server.name, "kanban");
        assert_eq!(server.command, "kanban");
        assert_eq!(server.args, vec!["serve".to_string()]);
        assert_eq!(
            profile.skills,
            Some(mirdan::install::Selector::Profile("kanban".to_string()))
        );
        assert!(profile.agents.is_none());
        assert!(!profile.statusline);
        assert!(!profile.preamble);
    }

    #[test]
    fn test_user_scope_selects_skills() {
        // Regression: `init user` must deploy the kanban-profile skills too.
        let profile = profile(InitScope::User);
        assert!(profile.mcp_server.is_some());
        assert_eq!(
            profile.skills,
            Some(mirdan::install::Selector::Profile("kanban".to_string())),
            "user scope must select the kanban-profile skills"
        );
    }

    #[test]
    fn test_local_scope_deploys_skills() {
        assert!(profile(InitScope::Local).skills.is_some());
    }

    #[test]
    fn test_register_all_registers_only_tool_lifecycle() {
        // Just the tool (`.kanban/` merge drivers). MCP registration and skill
        // deployment moved to the profile installer.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 1);
    }

    /// Regression for Bug 1 — `init user` deploys the kanban-profile skills
    /// (store + symlink) and registers the MCP server in the agent's global
    /// config. Drives the REAL `profile(InitScope::User)`.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_deploys_kanban_skills_and_registers_mcp() {
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
        assert_no_init_error("kanban user init", &results);

        UserScopeDeploy {
            home: &env.home_path(),
            server: "kanban",
            skills: KANBAN_SKILLS,
        }
        .assert();
    }

    /// Project-scope deploy rooted at an explicit `<root>`.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_deploys_kanban_skills_rooted() {
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
        assert_no_init_error("kanban project init", &results);

        ProjectScopeDeploy {
            root: &root,
            server: "kanban",
            skills: KANBAN_SKILLS,
        }
        .assert();
    }
}
