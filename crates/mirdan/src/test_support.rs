//! Public test-support helpers for driving the profile installer in isolation.
//!
//! Gated behind the `test-support` feature so it is compiled only for test
//! builds (the app CLIs enable it as a dev-dependency feature). These helpers
//! let a consumer run its **real** [`crate::install::Profile`] through
//! [`crate::install::init_profile`] against an isolated `$HOME` / explicit root,
//! then assert the store + symlink deploy mechanism landed every declared skill
//! and registered the MCP server.
//!
//! This is the single home for "install a profile in a hermetic environment"
//! so the per-CLI registry tests drive the production `profile(scope)` rather
//! than reconstructing it — closing the drift gap where a reconstructed profile
//! mirrors a bug in the real one and passes anyway.

use std::path::{Path, PathBuf};

use swissarmyhammer_common::lifecycle::{InitResult, InitScope, InitStatus};
use swissarmyhammer_common::reporter::NullReporter;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, EnvVarGuard, IsolatedTestEnvironment};

use crate::agents::AGENTS_CONFIG_ENV;
use crate::install::Selector;
use crate::tool_install::ToolInstall;

/// RAII guard that points mirdan's agent detection at a specific `agents.yaml`
/// via the `MIRDAN_AGENTS_CONFIG` env var, restoring the prior value on drop.
///
/// The env var is process-global, so tests using this guard must serialize
/// against each other (and against any CWD/HOME mutation) — apply
/// `#[serial_test::serial(cwd)]` at the call site.
#[derive(Debug)]
pub struct MirdanConfigGuard {
    /// Restores [`AGENTS_CONFIG_ENV`] on drop. Held for that effect alone,
    /// never read.
    _env: EnvVarGuard,
}

impl MirdanConfigGuard {
    /// Set [`AGENTS_CONFIG_ENV`] to `path`, capturing the prior value.
    pub fn set(path: impl AsRef<Path>) -> Self {
        Self {
            _env: EnvVarGuard::set(AGENTS_CONFIG_ENV, path.as_ref()),
        }
    }
}

/// Write a single generic agent's `agents.yaml` under `root`, detecting `root`
/// itself and declaring relative project paths plus `home`-rooted global paths
/// for the artifact kinds a profile installs (skills, agents, `.mcp.json`,
/// settings, instructions). Returns the path to the written config.
///
/// The project skill dir is `<root>/.fake/skills`; the global skill dir is
/// `<home>/.fake/skills`; the global MCP config is `<home>/.fake/mcp.json`.
/// These match the constants in [`AgentLayout`].
pub fn write_single_agent_config(root: &Path, home: &Path) -> PathBuf {
    let agents_yaml = format!(
        r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "{home}/.fake/skills"
    agent_path: .fake/agents
    settings_path: .fake/settings.json
    instructions_path: .fake/CLAUDE.md
    detect:
      - dir: "{detect}"
    mcp_config:
      project_path: .mcp.json
      global_path: "{home}/.fake/mcp.json"
      servers_key: mcpServers
"#,
        detect = root.display(),
        home = home.display(),
    );
    let config_path = root.join("agents.yaml");
    std::fs::write(&config_path, agents_yaml).unwrap();
    config_path
}

/// Assert no [`InitResult`] in `results` has `Error` status, labelling failures
/// with `phase`.
pub fn assert_no_init_error(phase: &str, results: &[InitResult]) {
    assert!(
        results.iter().all(|r| r.status != InitStatus::Error),
        "{phase} must not error: {:?}",
        results
            .iter()
            .filter(|r| r.status == InitStatus::Error)
            .map(|r| (&r.name, &r.message))
            .collect::<Vec<_>>()
    );
}

/// Read a JSON file into a [`serde_json::Value`], panicking on any failure.
pub fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Assertions for a `User`-scope (global) profile install against an isolated
/// `$HOME` matching [`write_single_agent_config`].
#[derive(Debug)]
pub struct UserScopeDeploy<'a> {
    /// The isolated home the install wrote into.
    pub home: &'a Path,
    /// The MCP server name the profile registered.
    pub server: &'a str,
    /// Every skill the profile must have deployed.
    pub skills: &'a [&'a str],
}

impl UserScopeDeploy<'_> {
    /// Assert each declared skill landed in the global store (`~/.skills`) as a
    /// `SKILL.md` and is **symlinked** (not copied) into the agent's global
    /// skill dir, and that the MCP server is registered in the agent's global
    /// config.
    pub fn assert(&self) {
        for skill in self.skills {
            let store = self.home.join(".skills").join(skill).join("SKILL.md");
            assert!(
                store.is_file(),
                "user scope: skill `{skill}` must be in the global ~/.skills store: {store:?}"
            );
            let link = self.home.join(".fake/skills").join(skill);
            let meta = std::fs::symlink_metadata(&link).unwrap_or_else(|e| {
                panic!("user scope: skill `{skill}` link must exist ({link:?}): {e}")
            });
            assert!(
                meta.file_type().is_symlink(),
                "user scope: skill `{skill}` must be a symlink, not a copy: {link:?}"
            );
        }
        let global_mcp = self.home.join(".fake/mcp.json");
        assert!(
            global_mcp.is_file()
                && read_json(&global_mcp)["mcpServers"][self.server]["command"] == self.server,
            "user scope: MCP server `{}` must be registered in the agent's global config",
            self.server
        );
    }
}

/// Assertions for a `Project`-scope profile install rooted at an explicit
/// `root` matching [`write_single_agent_config`].
#[derive(Debug)]
pub struct ProjectScopeDeploy<'a> {
    /// The explicit root the install was directed at.
    pub root: &'a Path,
    /// The MCP server name the profile registered.
    pub server: &'a str,
    /// Every skill the profile must have deployed.
    pub skills: &'a [&'a str],
}

impl ProjectScopeDeploy<'_> {
    /// Assert each declared skill landed in the project store (`<root>/.skills`)
    /// as a `SKILL.md` and is **symlinked** into the agent's project skill dir,
    /// and that the MCP server is registered in the project `.mcp.json`.
    pub fn assert(&self) {
        for skill in self.skills {
            let store = self.root.join(".skills").join(skill).join("SKILL.md");
            assert!(
                store.is_file(),
                "project scope: skill `{skill}` must be in the .skills store: {store:?}"
            );
            let link = self.root.join(".fake/skills").join(skill);
            let meta = std::fs::symlink_metadata(&link).unwrap_or_else(|e| {
                panic!("project scope: skill `{skill}` link must exist ({link:?}): {e}")
            });
            assert!(
                meta.file_type().is_symlink(),
                "project scope: skill `{skill}` must be a symlink, not a copy: {link:?}"
            );
        }
        let mcp = self.root.join(".mcp.json");
        assert!(
            mcp.is_file() && read_json(&mcp)["mcpServers"][self.server]["command"] == self.server,
            "project scope: MCP server `{}` must be registered in project .mcp.json",
            self.server
        );
    }
}

/// Assert `T`'s profile is exactly the tool shape: its own `<name> serve` MCP
/// server plus `expected_skills`, and none of the sah-only artifacts.
///
/// Every tool CLI declares that same shape, so the check lives here once rather
/// than once per CLI registry.
pub fn assert_tool_profile<T: ToolInstall>(expected_skills: &Selector) {
    let profile = T::profile();

    let server = profile
        .mcp_server
        .unwrap_or_else(|| panic!("{} profile must register an MCP server", T::SERVER_NAME));
    assert_eq!(server.name, T::SERVER_NAME);
    assert_eq!(server.command, T::SERVER_NAME);
    assert_eq!(server.args, vec!["serve".to_string()]);

    assert_eq!(profile.skills.as_ref(), Some(expected_skills));
    assert!(profile.agents.is_none());
    assert!(profile.validators.is_none());
    assert!(!profile.statusline);
    assert!(!profile.edit_redirect);
}

/// Assert `T` registers exactly `expected` genuine tool-lifecycle components —
/// the concerns that stay outside the profile installer.
pub fn assert_tool_component_count<T: ToolInstall>(expected: usize) {
    assert_eq!(
        T::component_registry().len(),
        expected,
        "{} must register exactly {expected} lifecycle component(s)",
        T::SERVER_NAME
    );
}

/// Drive `T`'s real `init` then `deinit` at `scope` in a hermetic environment,
/// asserting the install deployed every skill in `skills` plus the MCP server,
/// and that the matching `deinit` removed both again.
///
/// `$HOME`, the working directory, and the agent config are all redirected at
/// temporary locations, so nothing touches the developer's machine. Those
/// redirections are process-global: apply `#[serial_test::serial(cwd)]` at the
/// call site.
pub fn assert_tool_lifecycle_round_trip<T: ToolInstall>(scope: InitScope, skills: &[&str]) {
    let env = IsolatedTestEnvironment::new().unwrap();
    let home = env.home_path();
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&root).unwrap();
    let config_path = write_single_agent_config(&root, &home);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    // A user-scope install lands in `$HOME` and takes no explicit root; project
    // and local scope are rooted at `<root>` so they never read the CWD.
    let explicit_root = match scope {
        InitScope::User => None,
        _ => Some(root.as_path()),
    };
    // Where the fake agent layout puts what the install deployed.
    let (skill_dir, mcp_path) = match scope {
        InitScope::User => (home.join(".fake/skills"), home.join(".fake/mcp.json")),
        _ => (root.join(".fake/skills"), root.join(".mcp.json")),
    };

    let init_results = T::init(scope, explicit_root, &NullReporter);
    assert_no_init_error(&format!("{} init", T::SERVER_NAME), &init_results);
    match scope {
        InitScope::User => UserScopeDeploy {
            home: &home,
            server: T::SERVER_NAME,
            skills,
        }
        .assert(),
        _ => ProjectScopeDeploy {
            root: &root,
            server: T::SERVER_NAME,
            skills,
        }
        .assert(),
    }

    let deinit_results = T::deinit(scope, explicit_root, &NullReporter);
    assert_no_init_error(&format!("{} deinit", T::SERVER_NAME), &deinit_results);
    for skill in skills {
        assert!(
            !skill_dir.join(skill).exists(),
            "{} deinit must remove the `{skill}` skill link",
            T::SERVER_NAME
        );
    }
    let mcp = read_json(&mcp_path);
    assert!(
        mcp["mcpServers"][T::SERVER_NAME].is_null(),
        "{} deinit must unregister the MCP server: {mcp}",
        T::SERVER_NAME
    );
}
