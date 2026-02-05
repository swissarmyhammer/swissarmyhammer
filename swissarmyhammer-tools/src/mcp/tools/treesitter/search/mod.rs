//! Semantic search tool for finding similar code chunks

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    build_tool_schema, format_similar_chunks, open_workspace, resolve_workspace_path,
    schema_workspace_path_property,
};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;

/// Default number of results to return
const DEFAULT_TOP_K: usize = 10;

/// Default minimum similarity threshold (0.0-1.0)
const DEFAULT_MIN_SIMILARITY: f32 = 0.9;

/// MCP tool for semantic code search
#[derive(Default)]
pub struct TreesitterSearchTool;

impl TreesitterSearchTool {
    /// Creates a new instance of the TreesitterSearchTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(TreesitterSearchTool);

#[derive(Deserialize)]
struct SearchRequest {
    /// The text/code to search for similar chunks
    query: String,
    /// Maximum number of results to return
    #[serde(default = "default_top_k")]
    top_k: usize,
    /// Minimum similarity threshold 0.0-1.0
    #[serde(default = "default_min_similarity")]
    min_similarity: f32,
    /// Workspace path to search in (default: current directory)
    path: Option<String>,
}

fn default_top_k() -> usize {
    DEFAULT_TOP_K
}

fn default_min_similarity() -> f32 {
    DEFAULT_MIN_SIMILARITY
}

#[async_trait]
impl McpTool for TreesitterSearchTool {
    fn name(&self) -> &'static str {
        "treesitter_search"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        build_tool_schema(
            vec![
                (
                    "query",
                    json!({"type": "string", "description": "The text or code snippet to search for similar chunks"}),
                ),
                (
                    "top_k",
                    json!({"type": "integer", "description": "Maximum number of results to return (default: 10)", "default": 10}),
                ),
                (
                    "min_similarity",
                    json!({"type": "number", "description": "Minimum cosine similarity threshold 0.0-1.0 (default: 0.9)", "default": 0.9}),
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
        let request: SearchRequest = BaseToolImpl::parse_arguments(arguments)?;
        let workspace_path = resolve_workspace_path(request.path.as_ref(), context);

        tracing::debug!(
            "Performing semantic search in {:?} for: {}",
            workspace_path,
            request.query
        );

        let workspace = open_workspace(&workspace_path).await?;

        let results = workspace
            .semantic_search(request.query.clone(), request.top_k, request.min_similarity)
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Semantic search failed: {}", e), None)
            })?;

        Ok(BaseToolImpl::create_success_response(
            format_similar_chunks(&results, "similar code chunks"),
        ))
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
        let tool = TreesitterSearchTool::new();
        assert_tool_basics(&tool, "treesitter_search", "semantic");
    }

    #[test]
    fn test_tool_default_creates_valid_instance() {
        let tool = TreesitterSearchTool;
        assert_tool_basics(&tool, "treesitter_search", "semantic");
    }

    #[test]
    fn test_schema_structure() {
        let tool = TreesitterSearchTool::new();
        assert_schema_is_object(&tool);
        assert_schema_has_properties(&tool, &["query", "top_k", "min_similarity", "path"]);
        assert_schema_has_required(&tool, &["query"]);
    }

    #[test]
    fn test_default_values() {
        assert_eq!(default_top_k(), DEFAULT_TOP_K);
        assert!((default_min_similarity() - DEFAULT_MIN_SIMILARITY).abs() < 0.001);
    }

    #[test]
    fn test_search_request_deserialization() {
        let json = json!({ "query": "fn main()" });
        let request: SearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "fn main()");
        assert_eq!(request.top_k, DEFAULT_TOP_K);
        assert!((request.min_similarity - DEFAULT_MIN_SIMILARITY).abs() < 0.001);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_search_request_with_all_fields() {
        let json = json!({
            "query": "fn main()",
            "top_k": 5,
            "min_similarity": 0.9,
            "path": "/some/path"
        });
        let request: SearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "fn main()");
        assert_eq!(request.top_k, 5);
        assert!((request.min_similarity - 0.9).abs() < 0.001);
        assert_eq!(request.path, Some("/some/path".to_string()));
    }

    #[tokio::test]
    async fn test_execute_no_leader_running() {
        let tool = TreesitterSearchTool::new();
        let mut extra_args = serde_json::Map::new();
        extra_args.insert("query".to_string(), json!("fn main()"));

        // With background indexing, Reader mode doesn't have embedding model
        // Semantic search requires Leader mode to embed query text
        let (result, _temp_dir) = execute_tool_with_temp_path(&tool, Some(extra_args)).await;
        assert!(
            result.is_err(),
            "Semantic search should fail in Reader mode (no embedding model)"
        );
    }
}
