//! Kanban init/deinit component registry.
//!
//! Defines the `Initializable` components for `kanban init` and `kanban deinit`,
//! and exposes `register_all` to populate an `InitRegistry` with them.
//!
//! The MCP-server registration lifecycle is owned by [`KanbanTool`] itself (in
//! `swissarmyhammer-tools`) via the `with_mcp_server` builder; the CLI only
//! injects the `kanban serve` entry. Per-scope agent-config merging lives in
//! `mirdan::install` (which dispatches to each agent's strategy), so neither
//! the CLI nor the tool reimplements scope logic — fixing the Local-scope bug
//! the bespoke per-agent loop previously had.
//!
//! Two components are registered:
//! - [`KanbanTool`] — MCP registration (via the injected `kanban serve` entry)
//!   plus `.kanban/` git merge driver setup. Owns its full lifecycle.
//! - [`KanbanSkillDeployment`] — resolves, renders, and deploys the builtin
//!   `kanban` skill to detected agent `.skills/` directories.

use std::collections::BTreeMap;

use mirdan::mcp_config::McpServerEntry;
use swissarmyhammer_common::lifecycle::InitRegistry;
use swissarmyhammer_tools::mcp::tools::kanban::KanbanTool;

use crate::commands::skill::KanbanSkillDeployment;

/// The MCP server name the tool registers under each agent's config. Matches
/// the binary and the server identity advertised by `commands/serve.rs`.
const SERVER_NAME: &str = "kanban";

/// Build the `kanban serve` MCP server entry the CLI injects into the tool.
fn kanban_mcp_entry() -> McpServerEntry {
    McpServerEntry {
        command: SERVER_NAME.to_string(),
        args: vec!["serve".to_string()],
        env: BTreeMap::new(),
    }
}

/// Register all kanban init/deinit components into the given registry.
///
/// Components are registered; `InitRegistry` sorts them by priority at
/// execution time. Actual execution order:
/// - priority 55: [`KanbanTool`] (MCP registration + `.kanban/` merge drivers —
///   owns its full lifecycle via the injected `kanban serve` entry)
/// - priority 20: [`KanbanSkillDeployment`] (builtin `kanban` skill deployment)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanTool::new().with_mcp_server(SERVER_NAME, kanban_mcp_entry()));
    registry.register(KanbanSkillDeployment);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use swissarmyhammer_common::lifecycle::{InitScope, Initializable};
    use swissarmyhammer_common::reporter::NullReporter;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};

    /// RAII guard that restores the `MIRDAN_AGENTS_CONFIG` env var on drop.
    struct MirdanConfigGuard {
        original: Option<String>,
    }

    impl MirdanConfigGuard {
        /// Set `MIRDAN_AGENTS_CONFIG` to `path`, capturing the prior value.
        fn set(path: &std::path::Path) -> Self {
            let original = std::env::var("MIRDAN_AGENTS_CONFIG").ok();
            std::env::set_var("MIRDAN_AGENTS_CONFIG", path);
            Self { original }
        }
    }

    impl Drop for MirdanConfigGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var("MIRDAN_AGENTS_CONFIG", value),
                None => std::env::remove_var("MIRDAN_AGENTS_CONFIG"),
            }
        }
    }

    /// Write a synthetic single-agent config detected via `project_dir`, with a
    /// relative project-level MCP config path so `init` writes a project
    /// `.mcp.json` under the cwd.
    fn write_agents_config(project_dir: &std::path::Path) -> std::path::PathBuf {
        let agents_yaml = format!(
            r#"agents:
  - id: claude-code
    name: Claude Code
    project_path: .fake/skills
    global_path: "~/.fake/skills"
    detect:
      - dir: "{detect}"
    settings_path: agent-config/settings.json
    mcp_config:
      project_path: .mcp.json
      servers_key: mcpServers
"#,
            detect = project_dir.display(),
        );
        let config_path = project_dir.join("agents.yaml");
        std::fs::write(&config_path, agents_yaml).expect("write agents.yaml");
        config_path
    }

    #[test]
    fn test_register_all_populates_registry() {
        // Two components: the tool (owning MCP + merge drivers) and skill
        // deployment. The former `kanban-mcp-registration` bespoke component
        // was folded into the tool's own lifecycle.
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 2);
    }

    /// Drives the tool's project-scope `init`/`deinit` (built exactly as
    /// `register_all` wires it) and verifies a `.mcp.json` kanban entry is
    /// created and then removed.
    ///
    /// Mutates both process-global CWD and `MIRDAN_AGENTS_CONFIG`, so it joins
    /// the crate-wide `cwd` and `env` serial groups and pins HOME to an
    /// isolated env.
    #[test]
    #[serial(cwd, env)]
    fn test_tool_init_and_deinit_register_success_path() {
        let env = IsolatedTestEnvironment::new().expect("create isolated test env");
        let project = env.home_path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let _cwd = CurrentDirGuard::new(&project).expect("chdir into project");
        let config_path = write_agents_config(&project);
        let _mirdan = MirdanConfigGuard::set(&config_path);

        // Build the tool exactly as `register_all` does and drive its lifecycle.
        let tool = KanbanTool::new().with_mcp_server(SERVER_NAME, kanban_mcp_entry());
        let reporter = NullReporter;
        let _ = tool.init(&InitScope::Project, &reporter);

        let mcp_json_path = project.join(".mcp.json");
        assert!(
            mcp_json_path.exists(),
            ".mcp.json should have been created by init"
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
        assert_eq!(parsed["mcpServers"]["kanban"]["command"], "kanban");

        let _ = tool.deinit(&InitScope::Project, &reporter);

        let parsed_after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_json_path).unwrap()).unwrap();
        assert!(
            parsed_after["mcpServers"]["kanban"].is_null(),
            "kanban entry should have been removed by deinit"
        );
    }
}
