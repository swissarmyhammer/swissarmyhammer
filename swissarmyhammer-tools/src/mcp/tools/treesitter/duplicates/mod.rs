//! Duplicate code detection tool using semantic similarity

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    build_tool_schema, format_duplicate_clusters, format_similar_chunks, open_workspace,
    resolve_workspace_path, schema_workspace_path_property,
};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

/// Default minimum similarity threshold for duplicate detection
const DEFAULT_MIN_SIMILARITY: f32 = 0.85;

/// Default minimum chunk size in bytes to consider
const DEFAULT_MIN_CHUNK_BYTES: usize = 100;

/// MCP tool for detecting duplicate code
#[derive(Default)]
pub struct TreesitterDuplicatesTool;

impl TreesitterDuplicatesTool {
    /// Creates a new instance of the TreesitterDuplicatesTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(TreesitterDuplicatesTool);

#[derive(Deserialize)]
struct DuplicatesRequest {
    /// Minimum similarity threshold 0.0-1.0 (default: 0.85)
    #[serde(default = "default_min_similarity")]
    min_similarity: f32,
    /// Minimum chunk size in bytes to consider (default: 100)
    #[serde(default = "default_min_chunk_bytes")]
    min_chunk_bytes: usize,
    /// Optional: find duplicates only for a specific file
    file: Option<String>,
    /// Workspace path (default: current directory)
    path: Option<String>,
}

fn default_min_similarity() -> f32 {
    DEFAULT_MIN_SIMILARITY
}

fn default_min_chunk_bytes() -> usize {
    DEFAULT_MIN_CHUNK_BYTES
}

#[async_trait]
impl McpTool for TreesitterDuplicatesTool {
    fn name(&self) -> &'static str {
        "treesitter_duplicates"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        build_tool_schema(
            vec![
                (
                    "min_similarity",
                    json!({"type": "number", "description": "Minimum cosine similarity threshold 0.0-1.0 (default: 0.85)"}),
                ),
                (
                    "min_chunk_bytes",
                    json!({"type": "integer", "description": "Minimum chunk size in bytes to consider (default: 100)"}),
                ),
                (
                    "file",
                    json!({"type": "string", "description": "Optional: find duplicates only for chunks in this specific file"}),
                ),
                ("path", schema_workspace_path_property()),
            ],
            None,
        )
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: DuplicatesRequest = BaseToolImpl::parse_arguments(arguments)?;
        let workspace_path = resolve_workspace_path(request.path.as_ref(), context);

        tracing::debug!(
            "Finding duplicate code in {:?} (min_similarity: {}, min_bytes: {})",
            workspace_path,
            request.min_similarity,
            request.min_chunk_bytes
        );

        let workspace = open_workspace(&workspace_path).await?;

        if let Some(file_path) = &request.file {
            let results = workspace
                .find_duplicates_in_file(PathBuf::from(file_path), request.min_similarity)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Duplicate detection failed: {}", e), None)
                })?;

            let header = format!("similar chunks to code in {}", file_path);
            Ok(BaseToolImpl::create_success_response(
                format_similar_chunks(&results, &header),
            ))
        } else {
            let clusters = workspace
                .find_all_duplicates(request.min_similarity, request.min_chunk_bytes)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Duplicate detection failed: {}", e), None)
                })?;

            Ok(BaseToolImpl::create_success_response(
                format_duplicate_clusters(&clusters),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::treesitter::shared::test_helpers::{
        assert_execute_succeeds_on_empty_workspace, assert_schema_has_properties,
        assert_schema_is_object, assert_tool_basics, execute_tool_with_temp_path,
    };

    #[test]
    fn test_constants() {
        assert!(DEFAULT_MIN_SIMILARITY > 0.0);
        assert!(DEFAULT_MIN_SIMILARITY <= 1.0);
        assert!(DEFAULT_MIN_CHUNK_BYTES > 0);
    }

    #[test]
    fn test_tool_basics() {
        let tool = TreesitterDuplicatesTool::new();
        assert_tool_basics(&tool, "treesitter_duplicates", "duplicate");
    }

    #[test]
    fn test_tool_default_creates_valid_instance() {
        let tool = TreesitterDuplicatesTool::default();
        assert_tool_basics(&tool, "treesitter_duplicates", "duplicate");
    }

    #[test]
    fn test_schema_structure() {
        let tool = TreesitterDuplicatesTool::new();
        assert_schema_is_object(&tool);
        assert_schema_has_properties(
            &tool,
            &["min_similarity", "min_chunk_bytes", "file", "path"],
        );
    }

    #[test]
    fn test_default_functions() {
        assert!((default_min_similarity() - DEFAULT_MIN_SIMILARITY).abs() < 0.001);
        assert_eq!(default_min_chunk_bytes(), DEFAULT_MIN_CHUNK_BYTES);
    }

    #[test]
    fn test_duplicates_request_defaults() {
        let json = json!({});
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert!((request.min_similarity - DEFAULT_MIN_SIMILARITY).abs() < 0.001);
        assert_eq!(request.min_chunk_bytes, DEFAULT_MIN_CHUNK_BYTES);
        assert!(request.file.is_none());
        assert!(request.path.is_none());
    }

    #[test]
    fn test_duplicates_request_with_all_fields() {
        let json = json!({
            "min_similarity": 0.9,
            "min_chunk_bytes": 200,
            "file": "src/main.rs",
            "path": "/some/project"
        });
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert!((request.min_similarity - 0.9).abs() < 0.001);
        assert_eq!(request.min_chunk_bytes, 200);
        assert_eq!(request.file, Some("src/main.rs".to_string()));
        assert_eq!(request.path, Some("/some/project".to_string()));
    }

    #[tokio::test]
    async fn test_execute_no_leader_running() {
        let tool = TreesitterDuplicatesTool::new();
        assert_execute_succeeds_on_empty_workspace(&tool, None).await;
    }

    #[tokio::test]
    async fn test_execute_with_nonexistent_file_returns_error() {
        // When searching for duplicates in a specific file that doesn't exist,
        // the tool should return an error
        let tool = TreesitterDuplicatesTool::new();
        let mut extra_args = serde_json::Map::new();
        extra_args.insert("file".to_string(), json!("nonexistent/file.rs"));
        let (result, _temp_dir) = execute_tool_with_temp_path(&tool, Some(extra_args)).await;
        // Returns error because the file doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_request_min_similarity_boundary_zero() {
        let json = json!({ "min_similarity": 0.0 });
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert!((request.min_similarity - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_request_min_similarity_boundary_one() {
        let json = json!({ "min_similarity": 1.0 });
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert!((request.min_similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_request_min_chunk_bytes_boundary() {
        let json = json!({ "min_chunk_bytes": 1 });
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.min_chunk_bytes, 1);
    }

    #[test]
    fn test_request_min_chunk_bytes_large_value() {
        let json = json!({ "min_chunk_bytes": 10000 });
        let request: DuplicatesRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.min_chunk_bytes, 10000);
    }
}
