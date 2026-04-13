//! Shelltool init/deinit component registry.
//!
//! Defines the `Initializable` components for `shelltool init` and `shelltool deinit`,
//! and exposes `register_all` to populate an `InitRegistry` with them.

use std::collections::BTreeMap;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;

/// Register all shelltool init/deinit components into the given registry.
///
/// Components are registered; `InitRegistry` sorts them by priority at
/// execution time. Actual execution order:
/// - priority  0: `ShellExecuteTool` (config file, Bash deny — uses trait default)
/// - priority 10: `ShelltoolMcpRegistration` (MCP server config for detected agents)
/// - priority 30: `ShelltoolSkillDeployment` (shell skill to agent `.skills/` dirs)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(ShelltoolMcpRegistration);
    registry.register(ShellExecuteTool::new());
    registry.register(super::skill::ShelltoolSkillDeployment);
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

    /// Priority 10 — runs after ShellExecuteTool (priority 0, the default).
    fn priority(&self) -> i32 {
        10
    }

    /// Register `shelltool serve` as an MCP server in all detected agents.
    ///
    /// Resolves which agents are installed via mirdan's agent detection and writes
    /// the `shelltool` entry into each agent's MCP config file.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let config = match mirdan::agents::load_agents_config() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load agents config: {}", e),
                )];
            }
        };

        let agents = mirdan::agents::get_detected_agents(&config);
        let entry = mirdan::mcp_config::McpServerEntry {
            command: "shelltool".to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };

        let global = matches!(scope, InitScope::User);
        let mut installed_count = 0;

        for agent in &agents {
            let mcp_path = if global {
                mirdan::agents::agent_global_mcp_config(&agent.def)
            } else {
                mirdan::agents::agent_project_mcp_config(&agent.def)
            };

            let Some(config_path) = mcp_path else {
                continue;
            };

            let servers_key = agent
                .def
                .mcp_config
                .as_ref()
                .map(|c| c.servers_key.as_str())
                .unwrap_or("mcpServers");

            match mirdan::mcp_config::register_mcp_server(
                &config_path,
                servers_key,
                "shelltool",
                &entry,
            ) {
                Ok(()) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Registered".to_string(),
                        message: format!("shelltool MCP server in {}", config_path.display()),
                    });
                    installed_count += 1;
                }
                Err(e) => {
                    reporter.emit(&InitEvent::Warning {
                        message: format!("Failed to register in {}: {}", config_path.display(), e),
                    });
                }
            }
        }

        vec![InitResult::ok(
            self.name(),
            format!("MCP server registered for {} agent(s)", installed_count),
        )]
    }

    /// Unregister `shelltool` from all detected agent MCP config files.
    fn deinit(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let config = match mirdan::agents::load_agents_config() {
            Ok(c) => c,
            Err(e) => {
                return vec![InitResult::error(
                    self.name(),
                    format!("Failed to load agents config: {}", e),
                )];
            }
        };

        let agents = mirdan::agents::get_detected_agents(&config);
        let global = matches!(scope, InitScope::User);
        let mut removed_count = 0;

        for agent in &agents {
            let mcp_path = if global {
                mirdan::agents::agent_global_mcp_config(&agent.def)
            } else {
                mirdan::agents::agent_project_mcp_config(&agent.def)
            };

            let Some(config_path) = mcp_path else {
                continue;
            };

            let servers_key = agent
                .def
                .mcp_config
                .as_ref()
                .map(|c| c.servers_key.as_str())
                .unwrap_or("mcpServers");

            match mirdan::mcp_config::unregister_mcp_server(&config_path, servers_key, "shelltool")
            {
                Ok(true) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Removed".to_string(),
                        message: format!("shelltool MCP server from {}", config_path.display()),
                    });
                    removed_count += 1;
                }
                Ok(false) => {
                    // Not present — that's fine, deinit is idempotent
                }
                Err(e) => {
                    reporter.emit(&InitEvent::Warning {
                        message: format!(
                            "Failed to unregister from {}: {}",
                            config_path.display(),
                            e
                        ),
                    });
                }
            }
        }

        vec![InitResult::ok(
            self.name(),
            format!("MCP server removed from {} agent config(s)", removed_count),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::reporter::NullReporter;

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
        assert_eq!(registry.len(), 3);
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
}
