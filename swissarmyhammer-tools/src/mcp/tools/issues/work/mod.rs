//! Issue workflow management tool for MCP operations
//!
//! This module provides the WorkIssueTool for switching to work on a specific issue.

use crate::mcp::responses::create_success_response;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::WorkIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for switching to work on an issue
#[derive(Default)]
pub struct WorkIssueTool;

impl WorkIssueTool {
    /// Creates a new instance of the WorkIssueTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for WorkIssueTool {
    fn name(&self) -> &'static str {
        "issue_work"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "work")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Issue name to work on"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: WorkIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Get the issue to determine its number for branch naming
        let issue_storage = context.issue_storage.read().await;
        let issue = match issue_storage.get_issue(request.name.as_str()).await {
            Ok(issue) => issue,
            Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue")),
        };

        // Create work branch with format: number_name
        let mut git_ops = context.git_ops.lock().await;
        let branch_name = issue.name.clone();

        match git_ops.as_mut() {
            Some(ops) => match ops.create_work_branch(&branch_name) {
                Ok(branch_name) => Ok(create_success_response(format!(
                    "Switched to work branch: {branch_name}"
                ))),
                Err(e) => Err(McpErrorHandler::handle_error(e, "create work branch")),
            },
            None => Err(McpError::internal_error(
                "Git operations not available".to_string(),
                None,
            )),
        }
    }
}
