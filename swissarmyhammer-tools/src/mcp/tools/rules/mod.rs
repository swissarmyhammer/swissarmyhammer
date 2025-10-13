pub mod check;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all rules-related MCP tools
pub fn register_rules_tools(registry: &mut ToolRegistry) {
    registry.register(check::RuleCheckTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_rules_tools() {
        let mut registry = ToolRegistry::new();
        register_rules_tools(&mut registry);

        // Verify the tool was registered
        assert!(registry.get_tool("rules_check").is_some());
    }
}
