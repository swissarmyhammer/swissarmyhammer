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

        // Get current branch first for validation and source tracking
        let mut git_ops = context.git_ops.lock().await;
        let current_branch = match git_ops.as_ref() {
            Some(ops) => match ops.current_branch() {
                Ok(branch) => branch,
                Err(e) => return Err(McpErrorHandler::handle_error(e, "get current branch")),
            },
            None => {
                return Err(McpError::internal_error(
                    "Git operations not available".to_string(),
                    None,
                ))
            }
        };

        // Enhanced validation for working on issues from other issue branches
        if current_branch.starts_with("issue/") {
            let error_msg = format!(
                "Cannot work on issue '{}' from issue branch '{}'. Issue branches cannot be used as source branches. Switch to a non-issue branch (like main, develop, or feature branch) first.",
                request.name.0, current_branch
            );
            return Err(McpError::invalid_params(error_msg, None));
        }

        let issue_storage = context.issue_storage.read().await;
        let issue_exists = issue_storage.get_issue(request.name.as_str()).await.is_ok();

        if issue_exists {
            // Issue exists - get it and use its stored source branch
            let issue = match issue_storage.get_issue(request.name.as_str()).await {
                Ok(issue) => issue,
                Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue")),
            };

            let branch_name = issue.name.clone();

            match git_ops.as_mut() {
                Some(ops) => match ops
                    .create_work_branch_with_source(&branch_name, Some(&issue.source_branch))
                {
                    Ok((branch_name, source_branch)) => Ok(create_success_response(format!(
                        "Switched to work branch: {branch_name} (created from {source_branch})"
                    ))),
                    Err(e) => Err(McpErrorHandler::handle_error(e, "create work branch")),
                },
                None => Err(McpError::internal_error(
                    "Git operations not available".to_string(),
                    None,
                )),
            }
        } else {
            // Issue doesn't exist - return error (consistent with other issue operations)
            Err(McpError::invalid_params(
                format!("Issue '{}' not found", request.name.0),
                None,
            ))
        }
    }
}
