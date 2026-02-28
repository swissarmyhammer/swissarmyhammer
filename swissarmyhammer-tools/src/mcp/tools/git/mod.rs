//! Git-related tools for MCP operations
//!
//! This module provides tools for interacting with git repositories using libgit2.
//! Tools in this category provide programmatic access to git operations without
//! requiring command-line git execution.
//!
//! ## Tool Implementation Pattern
//!
//! Each tool follows the standard MCP pattern:
//! - Individual module directory (e.g., `changes/`)
//! - `mod.rs` containing the tool implementation with `McpTool` trait
//! - Registration function that adds tools to the global registry
//!
//! ## Available Tools
//!
//! - **changes**: List files that have changed on a branch relative to its parent

pub mod changes;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all git-related tools with the registry
pub fn register_git_tools(registry: &mut ToolRegistry) {
    registry.register(changes::GitChangesTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_git_tools() {
        let mut registry = ToolRegistry::new();
        register_git_tools(&mut registry);

        assert!(registry.get_tool("git").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_git_tools_properties() {
        let mut registry = ToolRegistry::new();
        register_git_tools(&mut registry);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);

        let git_changes_tool = tools
            .iter()
            .find(|tool| tool.name == "git")
            .expect("git_changes tool should be registered");

        assert_eq!(git_changes_tool.name, "git");
        assert!(git_changes_tool.description.is_some());
        assert!(!git_changes_tool.input_schema.is_empty());
    }

    #[test]
    fn test_multiple_registrations() {
        let mut registry = ToolRegistry::new();

        register_git_tools(&mut registry);
        register_git_tools(&mut registry);

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("git").is_some());
    }

    #[test]
    fn test_git_tool_name_uniqueness() {
        let mut registry = ToolRegistry::new();
        register_git_tools(&mut registry);

        let tool_names = registry.list_tool_names();
        let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();

        assert_eq!(tool_names.len(), unique_names.len());
    }
}
