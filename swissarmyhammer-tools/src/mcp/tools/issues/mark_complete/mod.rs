//! Issue completion tool for MCP operations
//!
//! This module provides the MarkCompleteIssueTool for marking issues as complete through the MCP protocol.

use crate::mcp::responses::create_mark_complete_response;
use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use swissarmyhammer_common::SwissArmyHammerError;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::MarkCompleteRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_issues_config::Config;

/// Tool for marking issues as complete
#[derive(Default)]
pub struct MarkCompleteIssueTool;

impl MarkCompleteIssueTool {
    /// Creates a new instance of the MarkCompleteIssueTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for MarkCompleteIssueTool {
    fn name(&self) -> &'static str {
        "issue_mark_complete"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "mark_complete")
            .expect("Tool description should be available")
    }

    fn cli_name(&self) -> &'static str {
        "complete"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Issue name to mark as complete. Use 'current' to mark the current issue complete."
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
        let request: MarkCompleteRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate issue name is not empty
        McpValidation::validate_not_empty(request.name.as_str(), "issue name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;

        // Handle special parameters
        let issue_name = if request.name.0 == "current" {
            // Get current issue name from git branch
            let git_ops = context.git_ops.lock().await;
            match git_ops.as_ref() {
                Some(ops) => match ops.get_current_branch() {
                    Ok(Some(branch)) => {
                        let branch_str = branch.to_string();
                        let config = Config::global();
                        if let Some(issue_name) =
                            branch_str.strip_prefix(&config.issue_branch_prefix)
                        {
                            issue_name.to_string()
                        } else {
                            return Err(McpError::invalid_params(
                                format!("Not on an issue branch. Current branch: {branch_str}"),
                                None,
                            ));
                        }
                    }
                    Ok(None) => {
                        return Err(McpError::invalid_params(
                            "Not on any branch (detached HEAD)".to_string(),
                            None,
                        ));
                    }
                    Err(e) => {
                        return Err(McpErrorHandler::handle_error(
                            SwissArmyHammerError::Other { message: e.to_string() },
                            "get current branch",
                        ));
                    }
                },
                None => {
                    return Err(McpError::invalid_params(
                        "Git operations not available".to_string(),
                        None,
                    ));
                }
            }
        } else {
            request.name.0
        };

        let issue_storage = context.issue_storage.write().await;
        match issue_storage.complete_issue(&issue_name).await {
            Ok(issue) => {
                tracing::info!("Successfully marked issue '{}' as complete", issue.name);
                Ok(create_mark_complete_response(&issue))
            }
            Err(e) => Err(McpErrorHandler::handle_error(
                swissarmyhammer_common::SwissArmyHammerError::Other { message: e.to_string() },
                "mark issue complete",
            )),
        }
    }
}
