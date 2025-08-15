//! Web search tools for MCP operations
//!
//! This module provides web search tools that enable LLMs to perform web searches using SearXNG
//! metasearch engines. The tools provide privacy-respecting search capabilities with automatic
//! result fetching and content processing.

pub mod content_fetcher;
pub mod enhanced_search;
pub mod error_recovery;
pub mod health_checker;
pub mod instance_discovery;
pub mod instance_manager;
pub mod privacy;
pub mod search;
pub mod types;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all web search-related tools with the registry
///
/// This function registers web search tools with the provided registry.
/// The tools expose web search functionality that uses SearXNG for privacy-respecting
/// search operations with optional content fetching.
///
/// # Arguments
///
/// * `registry` - The tool registry to register the web search tools with
///
/// # Tools Registered
///
/// - `web_search`: Basic web search using SearXNG with optional content fetching
/// - `enhanced_web_search`: Enhanced web search with comprehensive error handling, circuit breaker protection, and graceful degradation
pub fn register_web_search_tools(registry: &mut ToolRegistry) {
    registry.register(search::WebSearchTool::new());
    registry.register(enhanced_search::EnhancedWebSearchTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_web_search_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_web_search_tools(&mut registry);

        assert_eq!(registry.len(), 2);
        assert!(registry.get_tool("web_search").is_some());
        assert!(registry.get_tool("enhanced_web_search").is_some());
    }

    #[test]
    fn test_web_search_tool_is_properly_named() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let web_search_tool = registry.get_tool("web_search").unwrap();
        assert_eq!(web_search_tool.name(), "web_search");
    }

    #[test]
    fn test_web_search_tool_has_description() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let web_search_tool = registry.get_tool("web_search").unwrap();
        assert!(!web_search_tool.description().is_empty());
        assert!(web_search_tool.description().contains("web search"));
    }

    #[test]
    fn test_web_search_tool_has_valid_schema() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let web_search_tool = registry.get_tool("web_search").unwrap();
        let schema = web_search_tool.schema();

        // Verify schema is a valid JSON object
        assert_eq!(schema["type"], "object");

        // Verify required fields
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["required"].is_array());
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("query".to_string())));
    }

    #[test]
    fn test_enhanced_web_search_tool_is_properly_named() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let enhanced_tool = registry.get_tool("enhanced_web_search").unwrap();
        assert_eq!(enhanced_tool.name(), "enhanced_web_search");
    }

    #[test]
    fn test_enhanced_web_search_tool_has_description() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let enhanced_tool = registry.get_tool("enhanced_web_search").unwrap();
        assert!(!enhanced_tool.description().is_empty());
        assert!(enhanced_tool.description().contains("comprehensive error handling"));
    }

    #[test]
    fn test_enhanced_web_search_tool_has_valid_schema() {
        let mut registry = ToolRegistry::new();
        register_web_search_tools(&mut registry);

        let enhanced_tool = registry.get_tool("enhanced_web_search").unwrap();
        let schema = enhanced_tool.schema();

        // Verify schema is a valid JSON object
        assert_eq!(schema["type"], "object");

        // Verify required fields
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["required"].is_array());
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("query".to_string())));
    }
}
