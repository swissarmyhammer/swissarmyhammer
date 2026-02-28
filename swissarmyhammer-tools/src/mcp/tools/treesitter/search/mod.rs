//! Semantic search tool for finding similar code chunks

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    format_similar_chunks, open_workspace, resolve_workspace_path,
};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Default number of results to return
const DEFAULT_TOP_K: usize = 10;

/// Default minimum similarity threshold (0.0-1.0)
const DEFAULT_MIN_SIMILARITY: f32 = 0.9;

/// Operation metadata for semantic code search
#[derive(Debug, Default)]
pub struct SearchCode;

static SEARCH_CODE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("The text or code snippet to search for similar chunks")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("top_k")
        .description("Maximum number of results to return (default: 10)")
        .param_type(ParamType::Integer),
    ParamMeta::new("min_similarity")
        .description("Minimum cosine similarity threshold 0.0-1.0 (default: 0.9)")
        .param_type(ParamType::Number),
    ParamMeta::new("path")
        .description("Workspace path (default: current directory)")
        .param_type(ParamType::String),
];

impl Operation for SearchCode {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "code"
    }
    fn description(&self) -> &'static str {
        "Semantic search for similar code chunks using embeddings"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SEARCH_CODE_PARAMS
    }
}

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

/// Execute a semantic code search operation
pub async fn execute_search(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
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
        .map_err(|e| McpError::internal_error(format!("Semantic search failed: {}", e), None))?;

    Ok(BaseToolImpl::create_success_response(
        format_similar_chunks(&results, "similar code chunks"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
