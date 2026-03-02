//! Duplicate code detection tool using semantic similarity

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    format_duplicate_clusters, format_similar_chunks, open_workspace, resolve_workspace_path,
};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use std::path::PathBuf;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Default minimum similarity threshold for duplicate detection
const DEFAULT_MIN_SIMILARITY: f32 = 0.85;

/// Default minimum chunk size in bytes to consider
const DEFAULT_MIN_CHUNK_BYTES: usize = 100;

/// Operation metadata for duplicate code detection
#[derive(Debug, Default)]
pub struct FindDuplicates;

static FIND_DUPLICATES_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("min_similarity")
        .description("Minimum cosine similarity threshold 0.0-1.0 (default: 0.85)")
        .param_type(ParamType::Number),
    ParamMeta::new("min_chunk_bytes")
        .description("Minimum chunk size in bytes to consider (default: 100)")
        .param_type(ParamType::Integer),
    ParamMeta::new("file")
        .description("Optional: find duplicates only for chunks in this specific file")
        .param_type(ParamType::String),
    ParamMeta::new("path")
        .description("Workspace path (default: current directory)")
        .param_type(ParamType::String),
];

impl Operation for FindDuplicates {
    fn verb(&self) -> &'static str {
        "find"
    }
    fn noun(&self) -> &'static str {
        "duplicates"
    }
    fn description(&self) -> &'static str {
        "Detect duplicate code clusters using semantic similarity analysis"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        FIND_DUPLICATES_PARAMS
    }
}

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

/// Execute a duplicate code detection operation
pub async fn execute_duplicates(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_constants() {
        const { assert!(DEFAULT_MIN_SIMILARITY > 0.0) };
        const { assert!(DEFAULT_MIN_SIMILARITY <= 1.0) };
        const { assert!(DEFAULT_MIN_CHUNK_BYTES > 0) };
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
