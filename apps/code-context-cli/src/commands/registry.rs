//! Code-context init/deinit profile + component registry.
//!
//! `code-context init` / `code-context deinit` install two kinds of thing:
//!
//! 1. **Profile artifacts** — the `code-context` MCP server registration and the
//!    builtin `code-context` + `explore` + `lsp` + `detected-projects` skills.
//!    These are declared once as a
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

/// The builtin skills code-context deploys, in deployment order: the
/// `code-context` tool skill, `explore` (the structural investigation workflow
/// it powers), `lsp` (LSP server diagnostics/install), and `detected-projects`
/// (project-type/build-command discovery).
pub const SKILL_NAMES: &[&str] = &["code-context", "explore", "lsp", "detected-projects"];

/// The declarative manifest of what `code-context init`/`deinit` install through
/// mirdan's profile installer: the `code-context serve` MCP server and the
/// builtin [`SKILL_NAMES`] skills. No agents.
///
/// Skills deploy at every scope, including `User` — a global install lands every
/// declared skill in the global store (`~/.skills` + the agent's global skill
/// dir), so `init user` is a full configuration. This matches sah's
/// `Selector::All`, which already deploys at every scope. The `scope` parameter
/// is retained for signature parity with the other consumers (and forwarded to
/// the installer by the caller), but no longer gates skill selection.
pub fn profile(_scope: InitScope) -> mirdan::install::Profile {
    mirdan::install::Profile {
        mcp_server: Some(mirdan::install::ProfileMcpServer::serve(SERVER_NAME)),
        skills: Some(skills_selector()),
        agents: None,
        validators: None,
        statusline: false,
        preamble: false,
    }
}

/// The skills-only selector ([`SKILL_NAMES`]), shared by [`profile`] and the
/// `code-context skill` subcommand.
pub fn skills_selector() -> mirdan::install::Selector {
    mirdan::install::Selector::Named(SKILL_NAMES.iter().map(|s| s.to_string()).collect())
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

    /// The exact four-skill set code-context deploys at every scope.
    const EXPECTED_SKILLS: &[&str] = &["code-context", "explore", "lsp", "detected-projects"];

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
        assert!(!profile.preamble);
    }

    #[test]
    fn test_user_scope_selects_skills() {
        // Regression: `init user` must deploy the full skill set too.
        let profile = profile(InitScope::User);
        assert!(profile.mcp_server.is_some());
        assert_eq!(
            profile.skills,
            Some(skills_selector()),
            "user scope must select the full code-context skill set"
        );
    }

    #[test]
    fn test_local_scope_deploys_skills() {
        assert!(profile(InitScope::Local).skills.is_some());
    }

    /// Regression for Bug 2 — the selector names exactly
    /// `code-context` + `explore` + `lsp` + `detected-projects`.
    #[test]
    fn test_skills_selector_names_the_four_skills() {
        assert_eq!(
            skills_selector(),
            mirdan::install::Selector::Named(
                EXPECTED_SKILLS.iter().map(|s| s.to_string()).collect()
            )
        );
    }

    #[test]
    fn test_register_all_registers_only_tool_lifecycle() {
        // Just the tool (`.code-context/` directory). MCP registration moved to
        // the profile installer.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 1);
    }

    /// Regression for Bugs 1 + 2 — `init user` deploys exactly the four declared
    /// skills (store + symlink, `explore` and `detected-projects` included) and
    /// registers the MCP server in the agent's global config. Drives the REAL
    /// `profile(InitScope::User)`.
    #[test]
    #[serial_test::serial(cwd)]
    fn user_scope_deploys_four_skills_and_registers_mcp() {
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
            skills: EXPECTED_SKILLS,
        }
        .assert();
    }

    /// Project-scope deploy rooted at an explicit `<root>` — the same exact
    /// four-skill set lands in the project store.
    #[test]
    #[serial_test::serial(cwd)]
    fn project_scope_deploys_four_skills_rooted() {
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
            skills: EXPECTED_SKILLS,
        }
        .assert();
    }
}
