//! Unified tree-sitter code intelligence tool for MCP operations
//!
//! This module provides a single `treesitter` tool that dispatches between operations:
//! - `search code`: Semantic search to find similar code chunks
//! - `query ast`: Execute tree-sitter S-expression queries on parsed files
//! - `find duplicates`: Detect duplicate code clusters across the project
//! - `get status`: Get the current status of the code index
//!
//! Follows the Operation pattern from `swissarmyhammer-operations`.
//!
//! ## Architecture
//!
//! The tool connects to a shared tree-sitter index leader process via RPC.
//! If no leader exists, the tool becomes the leader automatically.
//!
//! ## Supported Languages
//!
//! The index supports 30+ programming languages including:
//! - Systems: Rust, C, C++, Go, Zig
//! - Web: JavaScript, TypeScript, HTML, CSS
//! - Backend: Python, Java, Ruby, PHP, C#
//! - Functional: Haskell, OCaml, Elixir, Scala
//! - Config: JSON, YAML, TOML, Markdown

pub mod duplicates;
pub mod query;
pub mod schema;
pub mod search;
mod shared;
pub mod status;

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_operations::Operation;

use duplicates::FindDuplicates;
use query::QueryAst;
use search::SearchCode;
use status::GetStatus;

// Static operation instances for schema generation
static SEARCH_CODE: Lazy<SearchCode> = Lazy::new(SearchCode::default);
static QUERY_AST: Lazy<QueryAst> = Lazy::new(QueryAst::default);
static FIND_DUPLICATES: Lazy<FindDuplicates> = Lazy::new(FindDuplicates::default);
static GET_STATUS: Lazy<GetStatus> = Lazy::new(GetStatus::default);

static TREESITTER_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*SEARCH_CODE as &dyn Operation,
        &*QUERY_AST as &dyn Operation,
        &*FIND_DUPLICATES as &dyn Operation,
        &*GET_STATUS as &dyn Operation,
    ]
});

/// Unified tree-sitter tool providing code intelligence operations
#[derive(Default)]
pub struct TreesitterTool;

impl TreesitterTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for TreesitterTool {
    fn name(&self) -> &'static str {
        "treesitter"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_treesitter_mcp_schema(&TREESITTER_OPERATIONS)
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("treesitter")
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &TREESITTER_OPERATIONS;
        // SAFETY: TREESITTER_OPERATIONS is a static Lazy<Vec<...>> initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
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
            "search code" => search::execute_search(args, context).await,
            "query ast" => query::execute_query(args, context).await,
            "find duplicates" => duplicates::execute_duplicates(args, context).await,
            "get status" => status::execute_status(args, context).await,
            "" => {
                // Infer operation from present keys
                if arguments.contains_key("query") && !arguments.contains_key("files") && !arguments.contains_key("language") {
                    search::execute_search(args, context).await
                } else if arguments.contains_key("query") {
                    query::execute_query(args, context).await
                } else if arguments.contains_key("min_chunk_bytes") || arguments.contains_key("file") {
                    duplicates::execute_duplicates(args, context).await
                } else {
                    Err(McpError::invalid_params(
                        "Cannot determine operation. Provide 'op' field (\"search code\", \"query ast\", \"find duplicates\", or \"get status\").",
                        None,
                    ))
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'search code', 'query ast', 'find duplicates', 'get status'",
                    other
                ),
                None,
            )),
        }
    }
}

impl Doctorable for TreesitterTool {
    fn name(&self) -> &str {
        "Treesitter"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        Vec::new()
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

/// Register the unified treesitter tool with the registry
pub fn register_treesitter_tools(registry: &mut ToolRegistry) {
    registry.register(TreesitterTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_treesitter_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_treesitter_tools(&mut registry);

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("treesitter").is_some());
    }

    #[test]
    fn test_treesitter_tool_name() {
        let tool = TreesitterTool::new();
        assert_eq!(<TreesitterTool as McpTool>::name(&tool), "treesitter");
    }

    #[test]
    fn test_treesitter_tool_has_description() {
        let tool = TreesitterTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_treesitter_tool_has_operations() {
        let tool = TreesitterTool::new();
        let ops = tool.operations();
        assert_eq!(ops.len(), 4);
        assert!(ops.iter().any(|o| o.op_string() == "search code"));
        assert!(ops.iter().any(|o| o.op_string() == "query ast"));
        assert!(ops.iter().any(|o| o.op_string() == "find duplicates"));
        assert!(ops.iter().any(|o| o.op_string() == "get status"));
    }

    #[test]
    fn test_treesitter_tool_schema_has_op_field() {
        let tool = TreesitterTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("search code")));
        assert!(op_enum.contains(&serde_json::json!("query ast")));
        assert!(op_enum.contains(&serde_json::json!("find duplicates")));
        assert!(op_enum.contains(&serde_json::json!("get status")));
    }

    #[test]
    fn test_treesitter_tool_schema_has_operation_schemas() {
        let tool = TreesitterTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 4);
    }

    #[tokio::test]
    async fn test_treesitter_tool_unknown_op() {
        let tool = TreesitterTool::new();
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
    async fn test_treesitter_tool_missing_op_and_no_keys() {
        let tool = TreesitterTool::new();
        let context = crate::test_utils::create_test_context().await;

        let args = serde_json::Map::new();

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot determine operation"));
    }
}
