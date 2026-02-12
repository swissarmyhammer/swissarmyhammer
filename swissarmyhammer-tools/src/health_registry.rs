//! Health check registry for SwissArmyHammer tools
//!
//! This module provides a centralized collection of all tool health checks
//! that can be used by the `sah doctor` command.
//!
//! All MCP tools implement the Doctorable trait, so we can iterate over
//! registered MCP tools to collect their health checks.

use swissarmyhammer_common::health::HealthCheck;

use crate::mcp::tool_registry::ToolRegistry;
use crate::mcp::{
    register_file_tools, register_flow_tools, register_git_tools, register_shell_tools,
    register_todo_tools, register_web_tools,
};

/// Collect all tool health checks from MCP tools
///
/// Iterates over all registered MCP tools and collects their health checks.
/// This should be called by the `sah doctor` command to get a complete
/// picture of tool health.
///
/// Since all MCP tools implement Doctorable, we can iterate over registered
/// MCP tools and run their health checks.
///
/// # Returns
///
/// * `Vec<HealthCheck>` - All health checks from all registered tools
pub async fn collect_all_health_checks() -> Vec<HealthCheck> {
    // Create MCP tool registry and register all tools
    let mut tool_registry = ToolRegistry::new();

    // Register all MCP tools (same as server does)
    register_file_tools(&mut tool_registry).await;
    register_flow_tools(&mut tool_registry);
    register_git_tools(&mut tool_registry);
    register_shell_tools(&mut tool_registry);
    register_todo_tools(&mut tool_registry);
    register_web_tools(&mut tool_registry);

    // Collect health checks from all tools
    // Since McpTool extends Doctorable, all tools can provide health checks
    let mut all_checks = Vec::new();
    for tool in tool_registry.iter_tools() {
        if tool.is_applicable() {
            all_checks.extend(tool.run_health_checks());
        }
    }

    all_checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collect_all_health_checks() {
        let checks = collect_all_health_checks().await;

        // Should have at least some checks (web_search provides Chrome check)
        assert!(!checks.is_empty());

        // All checks should have proper fields
        for check in &checks {
            assert!(!check.category.is_empty());
            assert!(!check.name.is_empty());
            assert!(!check.message.is_empty());
        }
    }

    #[tokio::test]
    async fn test_web_search_chrome_check_included() {
        let checks = collect_all_health_checks().await;

        // Should have a Chrome check from web_search tool
        let chrome_check = checks
            .iter()
            .find(|c| c.name.contains("Chrome") && c.category == "tools");
        assert!(
            chrome_check.is_some(),
            "Should have Chrome check from web_search tool"
        );
    }
}
