//! Shelltool init/deinit component registry.
//!
//! Defines the `Initializable` components for `shelltool init` and `shelltool deinit`,
//! and exposes `register_all` to populate an `InitRegistry` with them.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mirdan::agents::DetectedAgent;
use mirdan::mcp_config::McpServerEntry;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// The MCP server name this component registers under each agent's config.
const SERVER_NAME: &str = "shelltool";

/// Default JSON key that holds the MCP servers map if an agent doesn't override it.
const DEFAULT_SERVERS_KEY: &str = "mcpServers";

/// Return the per-agent JSON key that contains its MCP servers map.
fn agent_servers_key(agent: &DetectedAgent) -> &str {
    agent
        .def
        .mcp_config
        .as_ref()
        .map(|c| c.servers_key.as_str())
        .unwrap_or(DEFAULT_SERVERS_KEY)
}

/// Resolve the MCP config file path for an agent given the install scope.
fn resolve_mcp_config_path(agent: &DetectedAgent, global: bool) -> Option<PathBuf> {
    if global {
        mirdan::agents::agent_global_mcp_config(&agent.def)
    } else {
        mirdan::agents::agent_project_mcp_config(&agent.def)
    }
}

/// Load detected agents, or return a pre-built error `InitResult` on failure.
fn load_detected_agents(component_name: &str) -> Result<Vec<DetectedAgent>, InitResult> {
    match mirdan::agents::load_agents_config() {
        Ok(c) => Ok(mirdan::agents::get_detected_agents(&c)),
        Err(e) => Err(InitResult::error(
            component_name,
            format!("Failed to load agents config: {}", e),
        )),
    }
}

/// Register `shelltool` into a single agent's MCP config file.
///
/// Returns `true` if the entry was written, `false` otherwise (including when
/// the agent has no config path or the write failed). Error and success events
/// are emitted through `reporter`.
fn register_agent(
    agent: &DetectedAgent,
    entry: &McpServerEntry,
    global: bool,
    reporter: &dyn InitReporter,
) -> bool {
    let Some(config_path) = resolve_mcp_config_path(agent, global) else {
        return false;
    };
    let servers_key = agent_servers_key(agent);
    match mirdan::mcp_config::register_mcp_server(&config_path, servers_key, SERVER_NAME, entry) {
        Ok(()) => {
            reporter.emit(&InitEvent::Action {
                verb: "Registered".to_string(),
                message: format!("shelltool MCP server in {}", config_path.display()),
            });
            true
        }
        Err(e) => {
            reporter.emit(&InitEvent::Warning {
                message: format!("Failed to register in {}: {}", config_path.display(), e),
            });
            false
        }
    }
}

/// Unregister `shelltool` from a single agent's MCP config file.
///
/// Returns `true` only when an entry was actually removed. `Ok(false)` (entry
/// already absent) and error paths are both handled silently / via reporter.
fn unregister_agent(agent: &DetectedAgent, global: bool, reporter: &dyn InitReporter) -> bool {
    let Some(config_path) = resolve_mcp_config_path(agent, global) else {
        return false;
    };
    let servers_key = agent_servers_key(agent);
    match mirdan::mcp_config::unregister_mcp_server(&config_path, servers_key, SERVER_NAME) {
        Ok(true) => {
            reporter.emit(&InitEvent::Action {
                verb: "Removed".to_string(),
                message: format!("shelltool MCP server from {}", config_path.display()),
            });
            true
        }
        Ok(false) => false,
        Err(e) => {
            reporter.emit(&InitEvent::Warning {
                message: format!("Failed to unregister from {}: {}", config_path.display(), e),
            });
            false
        }
    }
}

/// Register all shelltool init/deinit components into the given registry.
///
/// Components are registered in priority order:
/// - priority 10: `ShelltoolMcpRegistration` (MCP server config for detected agents)
/// - priority 20: `ShellExecuteTool` (config file, Bash deny, skill deployment)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(ShelltoolMcpRegistration);
    registry.register(ShellExecuteTool::new());
}

// ── ShelltoolMcpRegistration (priority 10) ───────────────────────────────────

/// Registers/unregisters the `shelltool serve` MCP server entry in all detected
/// agent config files (e.g. `.mcp.json`, `~/.claude.json`).
pub struct ShelltoolMcpRegistration;

impl Initializable for ShelltoolMcpRegistration {
    /// The component name shown in init/deinit output.
    fn name(&self) -> &str {
        "shelltool-mcp-registration"
    }

    /// Component category: configuration.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Priority 10 — runs before ShellExecuteTool (priority 20).
    fn priority(&self) -> i32 {
        10
    }

    /// Register `shelltool serve` as an MCP server in all detected agents.
    ///
    /// Resolves which agents are installed via mirdan's agent detection and writes
    /// the `shelltool` entry into each agent's MCP config file.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let agents = match load_detected_agents(self.name()) {
            Ok(a) => a,
            Err(err) => return vec![err],
        };
        let entry = McpServerEntry {
            command: SERVER_NAME.to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };
        let global = matches!(scope, InitScope::User);
        let installed_count = agents
            .iter()
            .filter(|agent| register_agent(agent, &entry, global, reporter))
            .count();
        vec![InitResult::ok(
            self.name(),
            format!("MCP server registered for {} agent(s)", installed_count),
        )]
    }

    /// Unregister `shelltool` from all detected agent MCP config files.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let agents = match load_detected_agents(self.name()) {
            Ok(a) => a,
            Err(err) => return vec![err],
        };
        let global = matches!(scope, InitScope::User);
        let removed_count = agents
            .iter()
            .filter(|agent| unregister_agent(agent, global, reporter))
            .count();
        vec![InitResult::ok(
            self.name(),
            format!("MCP server removed from {} agent config(s)", removed_count),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use swissarmyhammer_common::reporter::NullReporter;
    use tempfile::TempDir;

    /// Serializes tests that mutate process-global environment state
    /// (`env::current_dir`, `MIRDAN_AGENTS_CONFIG`). Tests that only
    /// read these values are not protected — they tolerate both Ok and
    /// Error outcomes and so are robust to races with the mutating tests.
    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    /// Acquire the env lock, recovering from any prior poisoning.
    ///
    /// A poisoned lock just means a previous test panicked while holding
    /// it; the guarded data is `()` so there is nothing to corrupt.
    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// RAII guard that restores `env::current_dir` on drop.
    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        /// Capture the current working directory so it can be restored later.
        fn capture() -> Self {
            Self {
                original: env::current_dir().expect("current_dir must be readable"),
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            // Best-effort restore; ignore errors during unwind.
            let _ = env::set_current_dir(&self.original);
        }
    }

    /// RAII guard that restores the `MIRDAN_AGENTS_CONFIG` env var on drop.
    struct MirdanConfigGuard {
        original: Option<String>,
    }

    impl MirdanConfigGuard {
        /// Capture the current `MIRDAN_AGENTS_CONFIG` value so it can be restored later.
        fn capture() -> Self {
            Self {
                original: env::var("MIRDAN_AGENTS_CONFIG").ok(),
            }
        }
    }

    impl Drop for MirdanConfigGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => env::set_var("MIRDAN_AGENTS_CONFIG", value),
                None => env::remove_var("MIRDAN_AGENTS_CONFIG"),
            }
        }
    }

    #[test]
    fn test_shelltool_mcp_registration_name_and_priority() {
        let component = ShelltoolMcpRegistration;
        assert_eq!(component.name(), "shelltool-mcp-registration");
        assert_eq!(component.category(), "configuration");
        assert_eq!(component.priority(), 10);
    }

    #[tokio::test]
    async fn test_register_all_populates_registry() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_init_returns_ok_result() {
        let component = ShelltoolMcpRegistration;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        // Should return exactly one result with Ok or Error status (depending on env)
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_deinit_returns_ok_result() {
        let component = ShelltoolMcpRegistration;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }

    /// Exercises the `global { agent_global_mcp_config(..) }` arm of `init`
    /// by invoking it with `InitScope::User`. Regardless of which agents
    /// mirdan detects, the global branch is always traversed.
    #[test]
    fn test_init_global_scope() {
        let component = ShelltoolMcpRegistration;
        let reporter = NullReporter;
        let results = component.init(&InitScope::User, &reporter);
        assert_eq!(results.len(), 1);
    }

    /// Exercises the `global { agent_global_mcp_config(..) }` arm of `deinit`
    /// by invoking it with `InitScope::User`.
    #[test]
    fn test_deinit_global_scope() {
        let component = ShelltoolMcpRegistration;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::User, &reporter);
        assert_eq!(results.len(), 1);
    }

    /// Exercises the register success path of `init` and the `Ok(true)` arm
    /// of `deinit` by driving through a synthetic agent whose MCP config
    /// lives under a tempdir.
    ///
    /// Uses `MIRDAN_AGENTS_CONFIG` to inject a single detected agent pointing
    /// at a relative `.mcp.json` path. With the cwd chdired into the tempdir,
    /// the register call creates `.mcp.json`, and the subsequent deinit call
    /// removes the `shelltool` entry from it.
    #[test]
    fn test_init_and_deinit_register_success_path() {
        let _guard = lock_env();
        let _cwd = CwdGuard::capture();
        let _mirdan_env = MirdanConfigGuard::capture();

        let tmp = TempDir::new().expect("create tempdir");
        let tmp_path = tmp
            .path()
            .canonicalize()
            .expect("canonicalize tempdir path");

        // Synthetic agents config: one agent, always-detected via the tempdir
        // itself, with a relative project-level MCP config path.
        let detect_dir = tmp_path.display();
        let agents_yaml = format!(
            r#"agents:
  - id: fake-agent
    name: Fake Agent
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    detect:
      - dir: "{}"
    mcp_config:
      project_path: .mcp.json
      servers_key: mcpServers
"#,
            detect_dir
        );
        let config_path = tmp_path.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).expect("write agents.yaml");

        env::set_var("MIRDAN_AGENTS_CONFIG", &config_path);
        env::set_current_dir(&tmp_path).expect("set_current_dir to tempdir");

        // Run init and verify .mcp.json was created with a shelltool entry.
        let component = ShelltoolMcpRegistration;
        let reporter = NullReporter;
        let init_results = component.init(&InitScope::Project, &reporter);
        assert_eq!(init_results.len(), 1);

        let mcp_json_path = tmp_path.join(".mcp.json");
        assert!(
            mcp_json_path.exists(),
            ".mcp.json should have been created by init"
        );
        let contents = std::fs::read_to_string(&mcp_json_path).expect("read .mcp.json");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("parse .mcp.json");
        assert!(
            parsed
                .get("mcpServers")
                .and_then(|s| s.get("shelltool"))
                .is_some(),
            "shelltool entry missing from .mcp.json: {}",
            contents
        );

        // Run deinit and verify the shelltool entry was removed (hits
        // the `Ok(true)` arm).
        let deinit_results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(deinit_results.len(), 1);

        let contents_after =
            std::fs::read_to_string(&mcp_json_path).expect("read .mcp.json after deinit");
        let parsed_after: serde_json::Value =
            serde_json::from_str(&contents_after).expect("parse .mcp.json after deinit");
        assert!(
            parsed_after
                .get("mcpServers")
                .and_then(|s| s.get("shelltool"))
                .is_none(),
            "shelltool entry should have been removed: {}",
            contents_after
        );
    }
}
