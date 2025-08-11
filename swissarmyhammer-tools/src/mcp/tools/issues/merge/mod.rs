//! Issue branch merging tool for MCP operations
//!
//! This module provides the MergeIssueTool for merging issue work branches.

use crate::mcp::responses::{create_error_response, create_success_response};
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::MergeIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for merging an issue work branch
#[derive(Default)]
pub struct MergeIssueTool;

impl MergeIssueTool {
    /// Creates a new instance of the MergeIssueTool
    pub fn new() -> Self {
        Self
    }

    /// Format the issue branch name with the standard prefix
    fn format_issue_branch_name(issue_name: &str) -> String {
        format!("issue/{issue_name}")
    }
}

#[async_trait]
impl McpTool for MergeIssueTool {
    fn name(&self) -> &'static str {
        "issue_merge"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "merge")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Issue name to merge"
                },
                "delete_branch": {
                    "type": "boolean",
                    "description": "Whether to delete the branch after merging",
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
        let request: MergeIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Get the issue to determine its details
        let issue_storage = context.issue_storage.read().await;
        let issue = match issue_storage.get_issue(request.name.as_str()).await {
            Ok(issue) => issue,
            Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue for merge")),
        };

        // Validate that the issue is completed before allowing merge
        if !issue.completed {
            return Ok(create_error_response(format!(
                "Issue '{}' must be completed before merging",
                request.name
            )));
        }

        // Note: Removed working directory check to allow merge operations when issue completion
        // creates uncommitted changes. The git merge command itself will handle conflicts appropriately.

        // Merge branch
        let mut git_ops = context.git_ops.lock().await;
        let issue_name = issue.name.clone();

        match git_ops.as_mut() {
            Some(ops) => {
                // First merge the branch
                match ops.merge_issue_branch(&issue_name) {
                    Ok(_) => {
                        let mut success_message =
                            format!("Merged work branch for issue {issue_name} to main");

                        // Get commit information after successful merge
                        let commit_info = match ops.get_last_commit_info() {
                            Ok(info) => {
                                let parts: Vec<&str> = info.split('|').collect();
                                if parts.len() >= 4 {
                                    format!(
                                        "\n\nMerge commit: {}\nMessage: {}\nAuthor: {}\nDate: {}",
                                        &parts[0][..8], // First 8 chars of hash
                                        parts[1],
                                        parts[2],
                                        parts[3]
                                    )
                                } else {
                                    format!("\n\nMerge commit: {info}")
                                }
                            }
                            Err(_) => String::new(),
                        };

                        // If delete_branch is true, delete the branch after successful merge
                        if request.delete_branch {
                            let branch_name = Self::format_issue_branch_name(&issue_name);
                            match ops.delete_branch(&branch_name) {
                                Ok(_) => {
                                    success_message
                                        .push_str(&format!(" and deleted branch {branch_name}"));
                                }
                                Err(e) => {
                                    success_message
                                        .push_str(&format!(" but failed to delete branch: {e}"));
                                }
                            }
                        }

                        success_message.push_str(&commit_info);
                        Ok(create_success_response(success_message))
                    }
                    Err(e) => {
                        // Add debug output to understand what's failing
                        tracing::error!("Merge failed for issue '{}': {}", issue_name, e);
                        Err(McpErrorHandler::handle_error(e, "merge issue branch"))
                    }
                }
            }
            None => Ok(create_error_response(
                "Git operations not available".to_string(),
            )),
        }
    }
}
