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
use swissarmyhammer::common::create_abort_file_current_dir;

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

        // Validate we're on an issue branch before proceeding
        let git_ops = context.git_ops.lock().await;
        match git_ops.as_ref() {
            Some(ops) => {
                match ops.current_branch() {
                    Ok(current_branch) => {
                        if !current_branch.starts_with("issue/") {
                            let abort_reason = format!(
                                "Cannot merge issue '{}' from branch '{}'. Merge operations must be performed from an issue branch. Switch to the appropriate issue branch first.",
                                request.name, current_branch
                            );

                            tracing::warn!("Invalid branch for merge operation: {}", abort_reason);

                            // Create abort file to signal workflow termination
                            if let Err(e) = create_abort_file_current_dir(&abort_reason) {
                                tracing::error!("Failed to create abort file: {}", e);
                                return Err(McpError::internal_error(
                                    format!("Invalid branch for merge and failed to create abort file: {}", e),
                                    None,
                                ));
                            }

                            return Err(McpError::invalid_params(abort_reason, None));
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to get current branch: {}", e);
                        tracing::error!("{}", error_msg);
                        return Err(McpErrorHandler::handle_error(
                            e,
                            "get current branch for merge validation",
                        ));
                    }
                }
            }
            None => {
                return Ok(create_error_response(
                    "Git operations not available for branch validation".to_string(),
                ));
            }
        }
        drop(git_ops); // Release the lock before proceeding

        // Get the issue to determine its details
        let issue_storage = context.issue_storage.read().await;
        let issue_info = match issue_storage.get_issue_info(request.name.as_str()).await {
            Ok(issue_info) => issue_info,
            Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue for merge")),
        };

        // Validate that the issue is completed before allowing merge
        if !issue_info.completed {
            return Ok(create_error_response(format!(
                "Issue '{}' must be completed before merging",
                request.name
            )));
        }

        // Note: Removed working directory check to allow merge operations when issue completion
        // creates uncommitted changes. The git merge command itself will handle conflicts appropriately.

        // Merge branch
        let mut git_ops = context.git_ops.lock().await;
        let issue_name = issue_info.issue.name.clone();

        match git_ops.as_mut() {
            Some(ops) => {
                // Merge the branch back using git merge-base to determine target
                match ops.merge_issue_branch_auto(&issue_name) {
                    Ok(target_branch) => {
                        let mut success_message = format!(
                            "Merged work branch for issue {issue_name} to {target_branch} (determined by git merge-base)"
                        );

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
