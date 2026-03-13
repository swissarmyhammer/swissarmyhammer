//! Ralph tool for persistent agent loop instructions
//!
//! Ralph stores per-session instructions as markdown files in `.ralph/`.
//! Used by Stop hooks to prevent Claude from stopping while work remains.
//!
//! ## Architecture
//!
//! The ralph tool has three submodules:
//! - `execute/` — MCP tool implementation (`RalphTool`, `McpTool` trait impl)
//! - `state.rs` — File-based state management (read/write `.ralph/<session_id>.md`)
//!
//! ## File Format
//!
//! Instructions are stored as markdown files with YAML frontmatter:
//! ```markdown
//! ---
//! instruction: Implement all kanban cards
//! iteration: 3
//! max_iterations: 50
//! ---
//!
//! Agent notes go here.
//! ```

pub mod execute;
pub mod state;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all ralph-related tools with the registry
///
/// This function registers the `RalphTool` which provides persistent
/// instruction management for agent Stop hooks.
///
/// # Arguments
///
/// * `registry` - The tool registry to register ralph tools with
pub fn register_ralph_tools(registry: &mut ToolRegistry) {
    registry.register(execute::RalphTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[tokio::test]
    async fn test_register_ralph_tools() {
        let mut registry = ToolRegistry::new();
        register_ralph_tools(&mut registry);
        assert!(registry.get_tool("ralph").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_ralph_tool_registered_with_properties() {
        let mut registry = ToolRegistry::new();
        register_ralph_tools(&mut registry);

        let tools = registry.list_tools();
        let ralph_tool = tools
            .iter()
            .find(|t| t.name == "ralph")
            .expect("ralph tool should be registered");

        assert_eq!(ralph_tool.name, "ralph");
        assert!(ralph_tool.description.is_some());
        assert!(!ralph_tool.input_schema.is_empty());
    }
}
