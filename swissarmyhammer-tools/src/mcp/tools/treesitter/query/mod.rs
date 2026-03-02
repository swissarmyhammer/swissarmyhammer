//! Tree-sitter query tool for AST pattern matching

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    format_query_matches, open_workspace, resolve_workspace_path,
};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use std::path::PathBuf;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Operation metadata for tree-sitter AST queries
#[derive(Debug, Default)]
pub struct QueryAst;

static QUERY_AST_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("Tree-sitter S-expression query pattern (e.g., '(function_item name: (identifier) @name)')")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("files")
        .description("Optional list of specific files to query")
        .param_type(ParamType::Array),
    ParamMeta::new("language")
        .description("Optional language filter (e.g., 'rust', 'python', 'javascript')")
        .param_type(ParamType::String),
    ParamMeta::new("path")
        .description("Workspace path (default: current directory)")
        .param_type(ParamType::String),
];

impl Operation for QueryAst {
    fn verb(&self) -> &'static str {
        "query"
    }
    fn noun(&self) -> &'static str {
        "ast"
    }
    fn description(&self) -> &'static str {
        "Execute tree-sitter S-expression queries to find AST patterns in code"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        QUERY_AST_PARAMS
    }
}

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

/// Execute a tree-sitter AST query operation
pub async fn execute_query(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
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
        .map_err(|e| McpError::internal_error(format!("Query execution failed: {}", e), None))?;

    Ok(BaseToolImpl::create_success_response(format_query_matches(
        &results,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
