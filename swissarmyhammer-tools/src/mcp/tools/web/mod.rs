//! Unified web tool for MCP operations
//!
//! This module provides a single `web` tool that dispatches between operations:
//! - `search url`: Search the web using DuckDuckGo with optional content fetching
//! - `fetch url`: Fetch a specific URL and convert HTML to markdown
//!
//! Follows the Operation pattern from `swissarmyhammer-operations`.

pub mod fetch;
pub mod schema;
pub mod search;

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_operations::Operation;

use fetch::FetchUrl;
use search::SearchUrl;

// Static operation instances for schema generation
static SEARCH_URL: Lazy<SearchUrl> = Lazy::new(SearchUrl::default);
static FETCH_URL: Lazy<FetchUrl> = Lazy::new(FetchUrl::default);

static WEB_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*SEARCH_URL as &dyn Operation,
        &*FETCH_URL as &dyn Operation,
    ]
});

/// Unified web tool providing search and fetch operations
#[derive(Default)]
pub struct WebTool;

impl WebTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for WebTool {
    fn name(&self) -> &'static str {
        "web"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_web_mcp_schema(&WEB_OPERATIONS)
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("web")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip the "op" key from arguments before passing to handlers
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "search url" => search::execute_search(args, context).await,
            "fetch url" => fetch::execute_fetch(args, context).await,
            "" => {
                // Infer operation from present keys
                if arguments.contains_key("query") {
                    search::execute_search(args, context).await
                } else if arguments.contains_key("url") {
                    fetch::execute_fetch(args, context).await
                } else {
                    Err(McpError::invalid_params(
                        "Cannot determine operation. Provide 'op' field (\"search url\" or \"fetch url\"), or include 'query' for search / 'url' for fetch.",
                        None,
                    ))
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'search url', 'fetch url'",
                    other
                ),
                None,
            )),
        }
    }
}

impl Doctorable for WebTool {
    fn name(&self) -> &str {
        "Web"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();

        let chrome_result = swissarmyhammer_web::detect_chrome();

        if chrome_result.found {
            let path = chrome_result.path.as_ref().unwrap();
            let method = chrome_result.detection_method.as_ref().unwrap();

            checks.push(HealthCheck::ok(
                "Chrome/Chromium Browser",
                format!("Found at {} (via {})", path.display(), method),
                self.category(),
            ));
        } else {
            let instructions = chrome_result.installation_instructions();

            checks.push(HealthCheck::warning(
                "Chrome/Chromium Browser",
                format!(
                    "Not found (required for web search)\nChecked {} locations",
                    chrome_result.paths_checked.len()
                ),
                Some(format!("Install Chrome/Chromium:\n{}", instructions)),
                self.category(),
            ));
        }

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

/// Register the unified web tool with the registry
pub fn register_web_tools(registry: &mut ToolRegistry) {
    registry.register(WebTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_web_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_web_tools(&mut registry);

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("web").is_some());
    }

    #[test]
    fn test_web_tool_name() {
        let tool = WebTool::new();
        assert_eq!(<WebTool as McpTool>::name(&tool), "web");
    }

    #[test]
    fn test_web_tool_has_description() {
        let tool = WebTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_web_tool_schema_has_op_field() {
        let tool = WebTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("search url")));
        assert!(op_enum.contains(&serde_json::json!("fetch url")));
    }

    #[test]
    fn test_web_tool_schema_has_operation_schemas() {
        let tool = WebTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 2);
    }

    #[tokio::test]
    async fn test_web_tool_unknown_op() {
        let tool = WebTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "op".to_string(),
            serde_json::Value::String("invalid op".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_web_tool_missing_op_and_no_keys() {
        let tool = WebTool::new();
        let context = crate::test_utils::create_test_context().await;

        let args = serde_json::Map::new();

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot determine operation"));
    }

    #[tokio::test]
    async fn test_web_tool_infer_search() {
        let tool = WebTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        // Should infer search and fail on empty query validation, not on dispatch
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_web_tool_infer_fetch() {
        let tool = WebTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("not-a-valid-url".to_string()),
        );

        // Should infer fetch and fail on URL validation, not on dispatch
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }
}
