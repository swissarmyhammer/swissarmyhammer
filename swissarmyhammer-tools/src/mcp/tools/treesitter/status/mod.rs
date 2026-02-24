//! Index status tool for checking tree-sitter index state

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::treesitter::shared::{
    build_tool_schema, resolve_workspace_path, schema_workspace_path_property,
};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_treesitter::Workspace;

/// Operation metadata for index status checking
#[derive(Debug, Default)]
pub struct GetStatus;

static GET_STATUS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("Workspace path (default: current directory)")
        .param_type(ParamType::String),
];

impl Operation for GetStatus {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "status"
    }
    fn description(&self) -> &'static str {
        "Get the current status of the tree-sitter code index"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_STATUS_PARAMS
    }
}

/// MCP tool for checking tree-sitter index status
#[derive(Default)]
pub struct TreesitterStatusTool;

impl TreesitterStatusTool {
    /// Creates a new instance of the TreesitterStatusTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(TreesitterStatusTool);

#[derive(Deserialize, Default)]
struct StatusRequest {
    /// Workspace path (default: current directory)
    path: Option<String>,
}

#[async_trait]
impl McpTool for TreesitterStatusTool {
    fn name(&self) -> &'static str {
        "treesitter_status"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        build_tool_schema(vec![("path", schema_workspace_path_property())], None)
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        execute_status(arguments, context).await
    }
}

/// Execute a status check operation
pub async fn execute_status(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: StatusRequest = BaseToolImpl::parse_arguments(arguments)?;
    let workspace_path = resolve_workspace_path(request.path.as_ref(), context);

    tracing::debug!("Checking tree-sitter index status for {:?}", workspace_path);

    // Open workspace - this will become leader or reader depending on lock state
    let workspace = match Workspace::open(&workspace_path).await {
        Ok(w) => w,
        Err(e) => {
            return Ok(BaseToolImpl::create_success_response(format!(
                "**Index Status: Not Available**\n\n\
                 Could not open tree-sitter workspace.\n\
                 Error: {}\n\n\
                 Workspace path: {}",
                e,
                workspace_path.display()
            )));
        }
    };

    let status = workspace.status().await.map_err(|e| {
        McpError::internal_error(format!("Failed to get index status: {}", e), None)
    })?;

    let files = workspace.list_files().await.unwrap_or_default();

    let ready_status = if status.is_ready { "Ready" } else { "Building" };
    let progress = if status.files_total > 0 {
        (status.files_indexed as f64 / status.files_total as f64 * 100.0) as u32
    } else {
        0
    };

    let output = format!(
        "**Index Status: {}**\n\n\
         | Metric | Value |\n\
         |--------|-------|\n\
         | Root Path | {} |\n\
         | Total Files | {} |\n\
         | Files Indexed | {} |\n\
         | Files Embedded | {} |\n\
         | Progress | {}% |\n\
         | Indexed Files Count | {} |\n",
        ready_status,
        status.root_path.display(),
        status.files_total,
        status.files_indexed,
        status.files_embedded,
        progress,
        files.len()
    );

    Ok(BaseToolImpl::create_success_response(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::treesitter::shared::test_helpers::{
        assert_schema_has_properties, assert_schema_is_object, assert_tool_basics,
        execute_tool_with_temp_path, setup_test_env,
    };
    use serde_json::json;

    #[test]
    fn test_tool_basics() {
        let tool = TreesitterStatusTool::new();
        assert_tool_basics(&tool, "treesitter_status", "status");
    }

    #[test]
    fn test_tool_default_creates_valid_instance() {
        let tool = TreesitterStatusTool;
        assert_tool_basics(&tool, "treesitter_status", "status");
    }

    #[test]
    fn test_schema_structure() {
        let tool = TreesitterStatusTool::new();
        assert_schema_is_object(&tool);
        assert_schema_has_properties(&tool, &["path"]);
    }

    #[test]
    fn test_status_request_default() {
        let request = StatusRequest::default();
        assert!(request.path.is_none());
    }

    #[test]
    fn test_status_request_deserialization_empty() {
        let json = json!({});
        let request: StatusRequest = serde_json::from_value(json).unwrap();
        assert!(request.path.is_none());
    }

    #[test]
    fn test_status_request_with_path() {
        let json = json!({ "path": "/some/project" });
        let request: StatusRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.path, Some("/some/project".to_string()));
    }

    #[tokio::test]
    async fn test_execute_on_empty_workspace_succeeds() {
        // With Workspace::open(), the tool becomes a leader automatically
        // and returns status information even for an empty workspace
        let tool = TreesitterStatusTool::new();
        let (result, _temp_dir) = execute_tool_with_temp_path(&tool, None).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        // Should have at least one content item
        assert!(!response.content.is_empty());
        // Verify it's a text response
        let content = &response.content[0];
        assert!(matches!(&content.raw, rmcp::model::RawContent::Text(_)));
    }

    #[tokio::test]
    async fn test_execute_with_empty_arguments() {
        let tool = TreesitterStatusTool::new();
        let (context, _temp_dir) = setup_test_env().await;
        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());
    }
}
