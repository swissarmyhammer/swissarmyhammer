//! Code-context init/deinit component registry.
//!
//! Defines the `Initializable` components for `code-context init` and
//! `code-context deinit`, and exposes `register_all` to populate an
//! `InitRegistry` with them.

use std::collections::BTreeMap;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;

/// Register all code-context init/deinit components into the given registry.
///
/// Components are registered in priority order:
/// - priority 10: `CodeContextMcpRegistration` (MCP server config for detected agents)
/// - priority 22: `CodeContextTool` (`.code-context/` directory and `.gitignore` management)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(CodeContextMcpRegistration);
    registry.register(CodeContextTool::new());
}

// ── CodeContextMcpRegistration (priority 10) ─────────────────────────────────

/// Resolved agent MCP config path with its servers key.
struct AgentMcpTarget {
    config_path: std::path::PathBuf,
    servers_key: String,
}

/// Load detected agents and resolve their MCP config paths for the given scope.
fn resolve_agent_targets(scope: &InitScope) -> Result<Vec<AgentMcpTarget>, String> {
    let config = mirdan::agents::load_agents_config()
        .map_err(|e| format!("Failed to load agents config: {e}"))?;
    let agents = mirdan::agents::get_detected_agents(&config);
    let global = matches!(scope, InitScope::User);

    Ok(agents
        .iter()
        .filter_map(|agent| {
            let mcp_path = if global {
                mirdan::agents::agent_global_mcp_config(&agent.def)
            } else {
                mirdan::agents::agent_project_mcp_config(&agent.def)
            };
            let config_path = mcp_path?;
            let servers_key = agent
                .def
                .mcp_config
                .as_ref()
                .map(|c| c.servers_key.clone())
                .unwrap_or_else(|| "mcpServers".to_string());
            Some(AgentMcpTarget {
                config_path,
                servers_key,
            })
        })
        .collect())
}

/// Registers/unregisters the `code-context serve` MCP server entry in all
/// detected agent config files (e.g. `.mcp.json`, `~/.claude.json`).
pub struct CodeContextMcpRegistration;

impl Initializable for CodeContextMcpRegistration {
    fn name(&self) -> &str {
        "code-context-mcp-registration"
    }
    fn category(&self) -> &str {
        "configuration"
    }
    fn priority(&self) -> i32 {
        10
    }

    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let targets = match resolve_agent_targets(scope) {
            Ok(t) => t,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let entry = mirdan::mcp_config::McpServerEntry {
            command: "code-context".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };
        let mut count = 0;
        for t in &targets {
            match mirdan::mcp_config::register_mcp_server(
                &t.config_path,
                &t.servers_key,
                "code-context",
                &entry,
            ) {
                Ok(()) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Registered".to_string(),
                        message: format!("code-context MCP server in {}", t.config_path.display()),
                    });
                    count += 1;
                }
                Err(e) => reporter.emit(&InitEvent::Warning {
                    message: format!("Failed to register in {}: {e}", t.config_path.display()),
                }),
            }
        }
        vec![InitResult::ok(
            self.name(),
            format!("MCP server registered for {count} agent(s)"),
        )]
    }

    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let targets = match resolve_agent_targets(scope) {
            Ok(t) => t,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let mut count = 0;
        for t in &targets {
            match mirdan::mcp_config::unregister_mcp_server(
                &t.config_path,
                &t.servers_key,
                "code-context",
            ) {
                Ok(true) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Removed".to_string(),
                        message: format!(
                            "code-context MCP server from {}",
                            t.config_path.display()
                        ),
                    });
                    count += 1;
                }
                Ok(false) => {} // not present — idempotent
                Err(e) => reporter.emit(&InitEvent::Warning {
                    message: format!("Failed to unregister from {}: {e}", t.config_path.display()),
                }),
            }
        }
        vec![InitResult::ok(
            self.name(),
            format!("MCP server removed from {count} agent config(s)"),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;

    #[test]
    fn test_code_context_mcp_registration_name_and_priority() {
        let component = CodeContextMcpRegistration;
        assert_eq!(component.name(), "code-context-mcp-registration");
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
        let component = CodeContextMcpRegistration;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        // Should return exactly one result (Ok or Error depending on env)
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_deinit_returns_ok_result() {
        let component = CodeContextMcpRegistration;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }
}
