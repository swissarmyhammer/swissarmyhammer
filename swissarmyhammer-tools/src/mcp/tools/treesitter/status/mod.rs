//! Index status tool for checking tree-sitter index state

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::treesitter::shared::resolve_workspace_path;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_treesitter::Workspace;

/// Operation metadata for index status checking
#[derive(Debug, Default)]
pub struct GetStatus;

static GET_STATUS_PARAMS: &[ParamMeta] = &[ParamMeta::new("path")
    .description("Workspace path (default: current directory)")
    .param_type(ParamType::String)];

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

#[derive(Deserialize, Default)]
struct StatusRequest {
    /// Workspace path (default: current directory)
    path: Option<String>,
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
    use serde_json::json;

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
}
