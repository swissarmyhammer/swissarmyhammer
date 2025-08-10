//! Search tools for MCP operations
//!
//! This module provides search tools that expose semantic search functionality through the MCP protocol.
//! It includes tools for indexing files and performing semantic search queries.

pub mod index;
pub mod query;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all search-related tools with the registry
///
/// This function registers both the search indexing and query tools with the provided registry.
/// These tools expose the semantic search functionality that uses vector embeddings and
/// TreeSitter parsing for code understanding.
///
/// # Arguments
///
/// * `registry` - The tool registry to register the search tools with
///
/// # Tools Registered
///
/// - `search_index`: Index files for semantic search using vector embeddings
/// - `search_query`: Perform semantic search queries across indexed files
pub fn register_search_tools(registry: &mut ToolRegistry) {
    registry.register(index::SearchIndexTool::new());
    registry.register(query::SearchQueryTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_search_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_search_tools(&mut registry);

        assert_eq!(registry.len(), 2);
        assert!(registry.get_tool("search_index").is_some());
        assert!(registry.get_tool("search_query").is_some());
    }

    #[test]
    fn test_search_tools_are_properly_named() {
        let mut registry = ToolRegistry::new();
        register_search_tools(&mut registry);

        let index_tool = registry.get_tool("search_index").unwrap();
        let query_tool = registry.get_tool("search_query").unwrap();

        assert_eq!(index_tool.name(), "search_index");
        assert_eq!(query_tool.name(), "search_query");
    }

    #[test]
    fn test_search_tools_have_descriptions() {
        let mut registry = ToolRegistry::new();
        register_search_tools(&mut registry);

        let index_tool = registry.get_tool("search_index").unwrap();
        let query_tool = registry.get_tool("search_query").unwrap();

        assert!(!index_tool.description().is_empty());
        assert!(!query_tool.description().is_empty());
        assert!(index_tool.description().contains("Index files"));
        assert!(query_tool.description().contains("semantic search"));
    }

    #[test]
    fn test_search_tools_have_valid_schemas() {
        let mut registry = ToolRegistry::new();
        register_search_tools(&mut registry);

        let index_tool = registry.get_tool("search_index").unwrap();
        let query_tool = registry.get_tool("search_query").unwrap();

        let index_schema = index_tool.schema();
        let query_schema = query_tool.schema();

        // Verify schemas are valid JSON objects
        assert_eq!(index_schema["type"], "object");
        assert_eq!(query_schema["type"], "object");

        // Verify required fields
        assert!(index_schema["properties"]["patterns"].is_object());
        assert!(query_schema["properties"]["query"].is_object());
    }
}
