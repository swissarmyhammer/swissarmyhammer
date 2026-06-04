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

use swissarmyhammer_common::lifecycle::{InitResult, InitStatus};

/// RAII guard that points mirdan's agent detection at a specific `agents.yaml`
/// via the `MIRDAN_AGENTS_CONFIG` env var, restoring the prior value on drop.
///
/// The env var is process-global, so tests using this guard must serialize
/// against each other (and against any CWD/HOME mutation) — apply
/// `#[serial_test::serial(cwd)]` at the call site.
pub struct MirdanConfigGuard {
    original: Option<String>,
}

impl MirdanConfigGuard {
    /// Set `MIRDAN_AGENTS_CONFIG` to `path`, capturing the prior value.
    pub fn set(path: &Path) -> Self {
        let original = std::env::var("MIRDAN_AGENTS_CONFIG").ok();
        std::env::set_var("MIRDAN_AGENTS_CONFIG", path);
        Self { original }
    }
}

impl Drop for MirdanConfigGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(v) => std::env::set_var("MIRDAN_AGENTS_CONFIG", v),
            None => std::env::remove_var("MIRDAN_AGENTS_CONFIG"),
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
