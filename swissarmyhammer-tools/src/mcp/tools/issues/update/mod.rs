//! Issue updating tool for MCP operations
//!
//! This module provides the UpdateIssueTool for updating existing issue content.

use crate::mcp::responses::create_success_response;
use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::UpdateIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for updating issue content
#[derive(Default)]
pub struct UpdateIssueTool;

impl UpdateIssueTool {
    /// Creates a new instance of the UpdateIssueTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for UpdateIssueTool {
    fn name(&self) -> &'static str {
        "issue_update"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "update")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Issue name to update"
                },
                "content": {
                    "type": "string",
                    "description": "New markdown content for the issue"
                },
                "append": {
                    "type": "boolean",
                    "description": "If true, append to existing content instead of replacing",
                    "default": false
                }
            },
            "required": ["name", "content"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: UpdateIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate issue name and content
        McpValidation::validate_not_empty(request.name.as_str(), "issue name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;
        McpValidation::validate_not_empty(&request.content, "issue content")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue content"))?;

        let issue_storage = context.issue_storage.write().await;

        // Handle append mode by reading existing content first
        let final_content = if request.append {
            match issue_storage.get_issue(request.name.as_str()).await {
                Ok(existing_issue) => {
                    format!("{}\n{}", existing_issue.content, request.content)
                }
                Err(_) => request.content, // If can't read existing, just use new content
            }
        } else {
            request.content
        };

        match issue_storage
            .update_issue(request.name.as_str(), final_content)
            .await
        {
            Ok(issue) => Ok(create_success_response(format!(
                "Updated issue {} ({})",
                issue.name,
                if request.append {
                    "append mode"
                } else {
                    "replace mode"
                }
            ))),
            Err(e) => Err(McpErrorHandler::handle_error(e, "update issue")),
        }
    }
}
