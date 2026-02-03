//! Tree-sitter query tool for AST pattern matching

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    build_tool_schema, format_query_matches, open_workspace, resolve_workspace_path,
    schema_workspace_path_property,
};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

/// MCP tool for executing tree-sitter queries
#[derive(Default)]
pub struct TreesitterQueryTool;

impl TreesitterQueryTool {
    /// Creates a new instance of the TreesitterQueryTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(TreesitterQueryTool);

#[derive(Deserialize)]
struct QueryRequest {
    /// Tree-sitter S-expression query pattern
    query: String,
    /// Optional list of files to search (searches all if not specified)
    files: Option<Vec<String>>,
    /// Optional language filter (e.g., "rust", "python")
    language: Option<String>,
    /// Workspace path (default: current directory)
    path: Option<String>,
}

#[async_trait]
impl McpTool for TreesitterQueryTool {
    fn name(&self) -> &'static str {
        "treesitter_query"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        build_tool_schema(
            vec![
                (
                    "query",
                    json!({"type": "string", "description": "Tree-sitter S-expression query pattern (e.g., '(function_item name: (identifier) @name)')"}),
                ),
                (
                    "files",
                    json!({"type": "array", "items": {"type": "string"}, "description": "Optional list of specific files to query"}),
                ),
                (
                    "language",
                    json!({"type": "string", "description": "Optional language filter (e.g., 'rust', 'python', 'javascript')"}),
                ),
                ("path", schema_workspace_path_property()),
            ],
            Some(vec!["query"]),
        )
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: QueryRequest = BaseToolImpl::parse_arguments(arguments)?;
        let workspace_path = resolve_workspace_path(request.path.as_ref(), context);

        tracing::debug!(
            "Executing tree-sitter query in {:?}: {}",
            workspace_path,
            request.query
        );

        let workspace = open_workspace(&workspace_path).await?;

        let files = request
            .files
            .map(|f| f.into_iter().map(PathBuf::from).collect());

        let results = workspace
            .tree_sitter_query(request.query.clone(), files, request.language.clone())
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Query execution failed: {}", e), None)
            })?;

        Ok(BaseToolImpl::create_success_response(format_query_matches(
            &results,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::treesitter::shared::test_helpers::{
        assert_schema_has_properties, assert_schema_has_required, assert_schema_is_object,
        assert_tool_basics, execute_tool_with_temp_path,
    };

    #[test]
    fn test_tool_basics() {
        let tool = TreesitterQueryTool::new();
        assert_tool_basics(&tool, "treesitter_query", "tree-sitter");
    }

    #[test]
    fn test_tool_default_creates_valid_instance() {
        let tool = TreesitterQueryTool::default();
        assert_tool_basics(&tool, "treesitter_query", "tree-sitter");
    }

    #[test]
    fn test_schema_structure() {
        let tool = TreesitterQueryTool::new();
        assert_schema_is_object(&tool);
        assert_schema_has_properties(&tool, &["query", "files", "language", "path"]);
        assert_schema_has_required(&tool, &["query"]);
    }

    #[test]
    fn test_query_request_deserialization() {
        let json = json!({ "query": "(function_item)" });
        let request: QueryRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "(function_item)");
        assert!(request.files.is_none());
        assert!(request.language.is_none());
        assert!(request.path.is_none());
    }

    #[test]
    fn test_query_request_with_all_fields() {
        let json = json!({
            "query": "(function_item name: (identifier) @name)",
            "files": ["src/main.rs", "src/lib.rs"],
            "language": "rust",
            "path": "/some/project"
        });
        let request: QueryRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "(function_item name: (identifier) @name)");
        assert_eq!(
            request.files,
            Some(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()])
        );
        assert_eq!(request.language, Some("rust".to_string()));
        assert_eq!(request.path, Some("/some/project".to_string()));
    }

    #[tokio::test]
    async fn test_execute_no_leader_running() {
        let tool = TreesitterQueryTool::new();
        let mut extra_args = serde_json::Map::new();
        extra_args.insert("query".to_string(), json!("(function_item)"));

        // With background indexing, Reader mode doesn't have parsed AST
        // Tree-sitter queries require Leader mode, which is no longer available
        let (result, _temp_dir) = execute_tool_with_temp_path(&tool, Some(extra_args)).await;
        assert!(
            result.is_err(),
            "Tree-sitter queries should fail in Reader mode (no parsed AST)"
        );
    }
}
