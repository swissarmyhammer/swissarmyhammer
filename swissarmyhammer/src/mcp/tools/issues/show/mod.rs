//! Issue show tool for MCP operations
//!
//! This module provides the ShowIssueTool for displaying specific issues through the MCP protocol.

use crate::config::Config;
use crate::issues::Issue;
use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};

/// Request structure for showing an issue
#[derive(Debug, Deserialize, Serialize)]
pub struct ShowIssueRequest {
    /// Name of the issue to show
    pub name: String,
    /// Show raw content only without formatting
    pub raw: Option<bool>,
}

/// Tool for showing issue details
#[derive(Default)]
pub struct ShowIssueTool;

impl ShowIssueTool {
    /// Creates a new instance of the ShowIssueTool
    pub fn new() -> Self {
        Self
    }

    /// Format issue status as colored emoji
    fn format_issue_status(completed: bool) -> &'static str {
        if completed {
            "✅ Completed"
        } else {
            "🔄 Active"
        }
    }

    /// Format issue for display
    fn format_issue_display(issue: &Issue) -> String {
        let status = Self::format_issue_status(issue.completed);

        let mut result = format!("{} Issue: {}\n", status, issue.name);
        result.push_str(&format!("📁 File: {}\n", issue.file_path.display()));
        result.push_str(&format!(
            "📅 Created: {}\n\n",
            issue.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
        result.push_str(&issue.content);

        result
    }
}

#[async_trait]
impl McpTool for ShowIssueTool {
    fn name(&self) -> &'static str {
        "issue_show"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "show")
            .unwrap_or("Display details of a specific issue by name")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the issue to show. Use 'current' to show the issue for the current git branch. Use 'next' to show the next pending issue."
                },
                "raw": {
                    "type": "boolean",
                    "description": "Show raw content only without formatting",
                    "default": false
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
        let request: ShowIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for issue show
        context
            .rate_limiter
            .check_rate_limit("unknown", "issue_show", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for issue show: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        // Validate issue name is not empty
        McpValidation::validate_not_empty(&request.name, "issue name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;

        tracing::debug!("Showing issue: {}", request.name);

        // Handle special parameters
        let issue = if request.name == "current" {
            // Get current issue name from git branch
            let git_ops = context.git_ops.lock().await;
            let issue_name = match git_ops.as_ref() {
                Some(ops) => match ops.current_branch() {
                    Ok(branch) => {
                        let config = Config::global();
                        if let Some(issue_name) = branch.strip_prefix(&config.issue_branch_prefix) {
                            issue_name.to_string()
                        } else {
                            return Ok(BaseToolImpl::create_success_response(format!(
                                "Not on an issue branch. Current branch: {branch}"
                            )));
                        }
                    }
                    Err(e) => {
                        return Err(McpErrorHandler::handle_error(e, "get current branch"));
                    }
                },
                None => {
                    return Ok(BaseToolImpl::create_success_response(
                        "Git operations not available",
                    ));
                }
            };

            // Find the current issue in storage
            let issue_storage = context.issue_storage.read().await;
            let all_issues = issue_storage
                .list_issues()
                .await
                .map_err(|e| McpErrorHandler::handle_error(e, "list issues"))?;

            all_issues
                .into_iter()
                .find(|i| i.name == issue_name)
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Issue '{issue_name}' not found"), None)
                })?
        } else if request.name == "next" {
            // Get next pending issue
            let issue_storage = context.issue_storage.read().await;
            match issue_storage.get_next_issue().await {
                Ok(Some(next_issue)) => next_issue,
                Ok(None) => {
                    return Ok(BaseToolImpl::create_success_response(
                        "No pending issues found. All issues are completed!",
                    ));
                }
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(e, "get next issue"));
                }
            }
        } else {
            // Regular issue name lookup
            let issue_storage = context.issue_storage.read().await;
            let all_issues = issue_storage
                .list_issues()
                .await
                .map_err(|e| McpErrorHandler::handle_error(e, "list issues"))?;

            all_issues
                .into_iter()
                .find(|i| i.name == request.name)
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Issue '{}' not found", request.name), None)
                })?
        };

        let response = if request.raw.unwrap_or(false) {
            issue.content
        } else {
            Self::format_issue_display(&issue)
        };

        tracing::info!("Showed issue {}", issue.name);
        Ok(BaseToolImpl::create_success_response(&response))
    }
}
