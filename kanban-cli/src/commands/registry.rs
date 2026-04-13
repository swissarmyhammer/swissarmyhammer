//! Kanban init/deinit component registry.
//!
//! Defines the `Initializable` components for `kanban init` and `kanban deinit`,
//! and exposes `register_all` to populate an `InitRegistry` with them.
//!
//! Currently two components are registered:
//! - priority 10: [`KanbanMcpRegistration`] — registers/unregisters the
//!   `kanban serve` MCP server entry in detected agent config files.
//! - priority 20: [`KanbanSkillDeployment`] — resolves, renders, and deploys
//!   the builtin `kanban` skill to detected agent `.skills/` directories.
//!
//! The ordering (MCP first, skill second) guarantees that MCP config is in
//! place before the skill is written, matching the invariant
//! [`KanbanSkillDeployment::priority`] documents.

use std::collections::BTreeMap;

use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, Initializable};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

use crate::commands::skill::KanbanSkillDeployment;

/// Register all kanban init/deinit components into the given registry.
///
/// Components are registered in priority order (lower runs first on init,
/// reverse on deinit):
/// - priority 10: [`KanbanMcpRegistration`] (MCP server config for detected agents)
/// - priority 20: [`KanbanSkillDeployment`] (builtin `kanban` skill deployment)
pub fn register_all(registry: &mut InitRegistry) {
    registry.register(KanbanMcpRegistration);
    registry.register(KanbanSkillDeployment);
}

// ── KanbanMcpRegistration (priority 10) ──────────────────────────────────────

/// MCP server name registered in agent config files. Matches the binary
/// and the server identity advertised by `commands/serve.rs`.
const MCP_SERVER_NAME: &str = "kanban";

/// Resolved agent MCP config path with its servers key.
///
/// Each detected agent may use a different JSON key for its server table
/// (e.g. `"mcpServers"`, `"mcp_servers"`), so we keep the key alongside
/// the config path when enumerating targets.
struct AgentMcpTarget {
    config_path: std::path::PathBuf,
    servers_key: String,
}

/// Load detected agents and resolve their MCP config paths for the given scope.
///
/// `InitScope::User` resolves to each agent's global MCP config; any other
/// scope resolves to the per-project path. Agents without an MCP config entry
/// in their agent definition are silently skipped.
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

/// Registers/unregisters the `kanban serve` MCP server entry in all detected
/// agent config files (e.g. `.mcp.json`, `~/.claude.json`).
///
/// This component only touches MCP registration; skill deployment is handled
/// separately by [`KanbanSkillDeployment`]. Keeping the two concerns in
/// distinct `Initializable` components lets `sah init` / `kanban init`
/// compose, reorder, or skip them independently.
pub struct KanbanMcpRegistration;

impl Initializable for KanbanMcpRegistration {
    /// The component name shown in init/deinit output.
    fn name(&self) -> &str {
        "kanban-mcp-registration"
    }

    /// Component category: configuration.
    fn category(&self) -> &str {
        "configuration"
    }

    /// Priority 10 — runs before [`KanbanSkillDeployment`] (priority 20) so
    /// that MCP config is in place before the skill is deployed.
    fn priority(&self) -> i32 {
        10
    }

    /// Install the `kanban` MCP server entry into every detected agent's
    /// config file.
    ///
    /// Returns exactly one [`InitResult`] summarizing the number of agents
    /// registered. Individual per-agent failures are reported through the
    /// `reporter` as [`InitEvent::Warning`] but do not short-circuit the
    /// overall init — one broken agent config should not stop the rest.
    fn init(&self, scope: &InitScope, reporter: &dyn InitReporter) -> Vec<InitResult> {
        let targets = match resolve_agent_targets(scope) {
            Ok(t) => t,
            Err(e) => return vec![InitResult::error(self.name(), e)],
        };
        let entry = mirdan::mcp_config::McpServerEntry {
            command: MCP_SERVER_NAME.to_string(),
            args: vec!["serve".to_string()],
            env: BTreeMap::new(),
        };
        let mut count = 0;
        for t in &targets {
            match mirdan::mcp_config::register_mcp_server(
                &t.config_path,
                &t.servers_key,
                MCP_SERVER_NAME,
                &entry,
            ) {
                Ok(()) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Registered".to_string(),
                        message: format!("kanban MCP server in {}", t.config_path.display()),
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

    /// Remove the `kanban` MCP server entry from every detected agent's
    /// config file.
    ///
    /// Missing entries are treated as a no-op (the operation is idempotent).
    /// Per-agent failures are reported via `reporter` warnings; the overall
    /// result is still `Ok` so `kanban deinit` can complete.
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
                MCP_SERVER_NAME,
            ) {
                Ok(true) => {
                    reporter.emit(&InitEvent::Action {
                        verb: "Removed".to_string(),
                        message: format!("kanban MCP server from {}", t.config_path.display()),
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
    fn test_kanban_mcp_registration_name_and_priority() {
        let component = KanbanMcpRegistration;
        assert_eq!(component.name(), "kanban-mcp-registration");
        assert_eq!(component.category(), "configuration");
        assert_eq!(component.priority(), 10);
    }

    #[test]
    fn test_register_all_populates_registry() {
        let mut registry = InitRegistry::new();
        register_all(&mut registry);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_init_returns_single_result() {
        let component = KanbanMcpRegistration;
        let reporter = NullReporter;
        let results = component.init(&InitScope::Project, &reporter);
        // Should return exactly one result (Ok or Error depending on env).
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_deinit_returns_single_result() {
        let component = KanbanMcpRegistration;
        let reporter = NullReporter;
        let results = component.deinit(&InitScope::Project, &reporter);
        assert_eq!(results.len(), 1);
    }
}
