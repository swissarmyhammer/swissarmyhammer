//! Shelltool init/deinit component registry.
//!
//! Defines the `Initializable` components for `shelltool init` and `shelltool deinit`,
//! and exposes `register_all` to populate an `InitRegistry` with them.
//!
//! The MCP-server registration lifecycle is owned by [`ShellExecuteTool`]
//! itself (in `swissarmyhammer-tools`) via the `with_mcp_server` builder; the
//! CLI only injects the `shelltool serve` entry. Per-scope agent-config
//! merging lives in `mirdan::mcp_config`, so neither the CLI nor the tool
//! reimplements scope logic.

use std::collections::BTreeMap;

use mirdan::mcp_config::McpServerEntry;
use swissarmyhammer_common::lifecycle::InitRegistry;
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// The MCP server name the tool registers under each agent's config.
const SERVER_NAME: &str = "shelltool";

/// Build the `shelltool serve` MCP server entry the CLI injects into the tool.
fn shelltool_mcp_entry() -> McpServerEntry {
    McpServerEntry {
        command: SERVER_NAME.to_string(),
        args: vec!["serve".to_string()],
        env: BTreeMap::new(),
    }
}

/// Register all shelltool init/deinit components into the given registry.
///
/// Components are registered; `InitRegistry` sorts them by priority at
/// execution time. Actual execution order:
/// - priority  0: `ShellExecuteTool` (MCP registration, Bash deny, config —
///   owns its full lifecycle via the injected `shelltool serve` entry)
/// - priority 30: `ShelltoolSkillDeployment` (shell skill to agent `.skills/` dirs)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(ShellExecuteTool::new().with_mcp_server(SERVER_NAME, shelltool_mcp_entry()));
    registry.register(super::skill::ShelltoolSkillDeployment);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::path::PathBuf;
    use swissarmyhammer_common::lifecycle::{InitScope, Initializable};
    use swissarmyhammer_common::reporter::NullReporter;
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_register_all_populates_registry() {
        // Two components now: the tool (owning MCP + Bash deny + config) and
        // skill deployment. The former `shelltool-mcp-registration` component
        // was folded into the tool's own lifecycle.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 2);
    }

    /// Drives the tool's project-scope `init`/`deinit` (as `register_all`
    /// wires it) and verifies a `.mcp.json` shelltool entry is created and
    /// then removed.
    ///
    /// Uses `MIRDAN_AGENTS_CONFIG` to inject a single detected agent pointing
    /// at a relative `.mcp.json` path. With the cwd chdired into the tempdir,
    /// `init` creates `.mcp.json`, and `deinit` removes the `shelltool` entry.
    ///
    /// This test mutates BOTH process-global CWD (`env::set_current_dir`) and
    /// the `MIRDAN_AGENTS_CONFIG` env var, so it joins BOTH the crate-wide
    /// `cwd` group (shared by every CWD-touching test in `skill.rs`,
    /// `logging.rs`, `main.rs`, `doctor.rs`) and the `env` group.
    #[tokio::test]
    #[serial(cwd, env)]
    async fn test_tool_init_and_deinit_register_success_path() {
        let _cwd = CwdGuard::capture();
        let _mirdan_env = MirdanConfigGuard::capture();

        let tmp = TempDir::new().expect("create tempdir");
        let tmp_path = tmp
            .path()
            .canonicalize()
            .expect("canonicalize tempdir path");

        // Synthetic agents config: one agent, always-detected via the tempdir
        // itself, with a relative project-level MCP config path.
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
            tmp_path.display()
        );
        let config_path = tmp_path.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).expect("write agents.yaml");

        env::set_var("MIRDAN_AGENTS_CONFIG", &config_path);
        env::set_current_dir(&tmp_path).expect("set_current_dir to tempdir");

        // Build the tool exactly as `register_all` does and drive its lifecycle.
        let tool = ShellExecuteTool::new().with_mcp_server(SERVER_NAME, shelltool_mcp_entry());
        let reporter = NullReporter;
        let _ = tool.init(&InitScope::Project, &reporter);

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

        let _ = tool.deinit(&InitScope::Project, &reporter);

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
