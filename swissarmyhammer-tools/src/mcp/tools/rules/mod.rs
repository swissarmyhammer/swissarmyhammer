pub mod check;
pub mod create;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all rules-related MCP tools
pub fn register_rules_tools(registry: &mut ToolRegistry) {
    registry.register(check::RuleCheckTool::new());
    registry.register(create::CreateRuleTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_rules_tools() {
        let mut registry = ToolRegistry::new();
        register_rules_tools(&mut registry);

        // Verify both tools are registered
        assert!(registry.get_tool("rules_check").is_some());
        assert!(registry.get_tool("rules_create").is_some());
    }
}
