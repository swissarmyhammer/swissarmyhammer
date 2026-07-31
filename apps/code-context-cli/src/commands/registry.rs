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

use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;

/// The MCP server name registered under each agent's config.
const SERVER_NAME: &str = "code-context";

/// The declarative manifest of what `code-context init`/`deinit` install through
/// mirdan's profile installer: the `code-context serve` MCP server and every
/// builtin skill. No agents.
///
/// Skills deploy at every scope, including `User` — a global install lands every
/// builtin skill in the global store (`~/.skills` + the agent's global skill
/// dir), so `init user` is a full configuration. Skill selection is not curated
/// per consumer: every consumer deploys `Selector::All`. The `scope` parameter
/// is retained for signature parity with the other consumers (and forwarded to
/// the installer by the caller), but does not gate skill selection.
pub fn profile(_scope: InitScope) -> mirdan::install::Profile {
    mirdan::install::Profile {
        mcp_server: Some(mirdan::install::ProfileMcpServer::serve(SERVER_NAME)),
        skills: Some(skills_selector()),
        agents: None,
        validators: None,
        statusline: false,
        preamble: false,
        edit_redirect: false,
    }
}

/// The skills-only selector, shared by [`profile`] and the `code-context skill`
/// subcommand. Every builtin skill, with no per-consumer curation.
pub fn skills_selector() -> mirdan::install::Selector {
    mirdan::install::Selector::All
}

/// Register the genuine tool-lifecycle components into `registry`.
///
/// Only [`CodeContextTool`] is registered — for its `.code-context/` directory
/// and `.gitignore` lifecycle. MCP registration is owned by the profile (see
/// [`profile`]), not a bespoke component.
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(CodeContextTool::new());
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

    /// A representative slice of the builtin skill set code-context deploys at
    /// every scope. `ci` is included because the old four-name selector
    /// withheld it.
    const PROBE_SKILLS: &[&str] = &["code-context", "explore", "lsp", "detected-projects", "ci"];

    #[test]
    fn test_profile_declares_mcp_and_skills_in_project_scope() {
        let profile = profile(InitScope::Project);
        let server = profile.mcp_server.expect("profile declares an MCP server");
        assert_eq!(server.name, "code-context");
        assert_eq!(server.command, "code-context");
        assert_eq!(server.args, vec!["serve".to_string()]);
        assert_eq!(profile.skills, Some(skills_selector()));
        assert!(profile.agents.is_none());
        assert!(!profile.statusline);
    }

    #[test]
    fn test_user_scope_selects_skills() {
        // Regression: `init user` must deploy every builtin skill too.
        let profile = profile(InitScope::User);
        assert!(profile.mcp_server.is_some());
        assert_eq!(
            profile.skills,
            Some(skills_selector()),
            "user scope must select every builtin skill"
        );
    }

    #[test]
    fn test_local_scope_deploys_skills() {
        assert!(profile(InitScope::Local).skills.is_some());
    }

    /// The selector deploys every builtin skill — curation by name or profile
    /// is gone.
    #[test]
    fn test_skills_selector_selects_every_builtin_skill() {
        assert_eq!(skills_selector(), mirdan::install::Selector::All);
    }

    #[test]
    fn test_register_all_registers_only_tool_lifecycle() {
        // Just the tool (`.code-context/` directory). MCP registration moved to
        // the profile installer.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 1);
    }

    /// Regression for Bugs 1 + 2 — `init user` deploys the builtin skills
    /// (store + symlink) and registers the MCP server in the agent's global
    /// config. Drives the REAL `profile(InitScope::User)`.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_deploys_skills_and_registers_mcp() {
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
        assert_no_init_error("code-context user init", &results);

        UserScopeDeploy {
            home: &env.home_path(),
            server: "code-context",
            skills: PROBE_SKILLS,
        }
        .assert();
    }

    /// Project-scope deploy rooted at an explicit `<root>` — the same builtin
    /// skill set lands in the project store.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_deploys_skills_rooted() {
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
        assert_no_init_error("code-context project init", &results);

        ProjectScopeDeploy {
            root: &root,
            server: "code-context",
            skills: PROBE_SKILLS,
        }
        .assert();
    }
}
