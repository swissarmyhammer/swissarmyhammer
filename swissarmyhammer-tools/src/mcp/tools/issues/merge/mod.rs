//! Issue branch merging tool for MCP operations
//!
//! This module provides the MergeIssueTool for merging issue work branches.

use crate::mcp::responses::create_success_response;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::MergeIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
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
                    "default": true
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
                match ops.get_current_branch() {
                    Ok(Some(current_branch)) => {
                        let current_branch_str = current_branch.to_string();
                        if !current_branch_str.starts_with("issue/") {
                            let abort_reason = format!(
                                "Cannot merge issue '{}' from branch '{}'. Merge operations must be performed from an issue branch. Switch to the appropriate issue branch first.",
                                request.name, current_branch_str
                            );

                            tracing::warn!("Invalid branch for merge operation: {}", abort_reason);

                            // Create abort file to signal workflow termination
                            create_abort_file_current_dir(&abort_reason);

                            return Err(McpError::invalid_params(abort_reason, None));
                        }
                    }
                    Ok(None) => {
                        let abort_reason = "Cannot determine current branch for merge validation. Repository may be in detached HEAD state.".to_string();

                        tracing::warn!(
                            "Unknown current branch for merge operation: {}",
                            abort_reason
                        );

                        // Create abort file to signal workflow termination
                        create_abort_file_current_dir(&abort_reason);

                        return Err(McpError::invalid_params(abort_reason, None));
                    }
                    Err(e) => {
                        let error_msg =
                            format!("Failed to get current branch for merge validation: {e}");
                        tracing::error!("{}", error_msg);

                        // Create abort file to signal workflow termination
                        create_abort_file_current_dir(&error_msg);

                        return Err(McpError::invalid_params(error_msg, None));
                    }
                }
            }
            None => {
                let error_msg = "Git operations not available for branch validation".to_string();
                tracing::error!("{}", error_msg);

                // Create abort file to signal workflow termination
                create_abort_file_current_dir(&error_msg);

                return Err(McpError::invalid_params(error_msg, None));
            }
        }
        drop(git_ops); // Release the lock before proceeding

        // Get the issue to determine its details
        let issue_storage = context.issue_storage.read().await;
        let issue_info = match issue_storage.get_issue_info(request.name.as_str()).await {
            Ok(issue_info) => issue_info,
            Err(e) => {
                let error_msg = format!("Failed to get issue '{}' for merge: {}", request.name, e);
                tracing::error!("{}", error_msg);

                // Create abort file to signal workflow termination
                create_abort_file_current_dir(&error_msg);

                return Err(McpError::invalid_params(error_msg, None));
            }
        };

        // Auto-complete the issue if it's not already completed
        if !issue_info.completed {
            tracing::info!(
                "Issue '{}' is not completed, marking as complete before merge",
                request.name
            );

            // Use the mark_complete tool to complete the issue
            use crate::mcp::tools::issues::mark_complete::MarkCompleteIssueTool;
            use serde_json::json;

            let mut args = serde_json::Map::new();
            args.insert("name".to_string(), json!(request.name));

            let mark_complete_tool = MarkCompleteIssueTool;
            mark_complete_tool
                .execute(args, context)
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!(
                            "Failed to auto-complete issue '{}' before merge: {}",
                            request.name, e
                        ),
                        None,
                    )
                })?;

            tracing::info!("Successfully marked issue '{}' as complete", request.name);
        }

        // Note: Removed working directory check to allow merge operations when issue completion
        // creates uncommitted changes. The git merge command itself will handle conflicts appropriately.

        // Merge branch
        let mut git_ops = context.git_ops.lock().await;
        let issue_name = issue_info.issue.name.clone();

        match git_ops.as_mut() {
            Some(ops) => {
                // First, determine where the issue branch should be merged back to using git merge-base
                let issue_branch_name = format!("issue/{}", issue_name);
                let issue_branch_obj = swissarmyhammer_git::BranchName::new(&issue_branch_name)
                    .map_err(|e| {
                        McpError::internal_error(format!("Invalid issue branch name: {}", e), None)
                    })?;
                let target_branch = ops
                    .find_merge_target_for_issue(&issue_branch_obj)
                    .unwrap_or_else(|_| ops.main_branch().unwrap_or_else(|_| "main".to_string()));

                // Switch to the target branch first
                let target_branch_obj = swissarmyhammer_git::BranchName::new(&target_branch)
                    .map_err(|e| {
                        McpError::internal_error(format!("Invalid target branch name: {}", e), None)
                    })?;
                ops.checkout_branch(&target_branch_obj).map_err(|e| {
                    McpError::internal_error(
                        format!(
                            "Failed to checkout target branch '{}': {}",
                            target_branch, e
                        ),
                        None,
                    )
                })?;

                // Now merge the issue branch into the current (target) branch
                let issue_branch = swissarmyhammer_git::BranchName::new(&issue_branch_name)
                    .map_err(|e| {
                        McpError::internal_error(format!("Invalid branch name: {}", e), None)
                    })?;
                match ops.merge_branch(&issue_branch) {
                    Ok(()) => {
                        let source_branch = Self::format_issue_branch_name(&issue_name);
                        tracing::info!(
                            "Successfully merged issue branch '{}' to target branch '{}'",
                            source_branch,
                            target_branch
                        );

                        let mut success_message = format!(
                            "Merged work branch for issue {issue_name} to {target_branch} (determined by git merge-base)"
                        );

                        // Get commit information after successful merge
                        let commit_info = match ops.get_latest_commit() {
                            Ok(info) => {
                                format!(
                                    "\n\nMerge commit: {}\nMessage: {}\nAuthor: {} <{}>\nDate: {}",
                                    info.short_hash,
                                    info.message.lines().next().unwrap_or(""),
                                    info.author,
                                    info.author_email,
                                    info.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
                                )
                            }
                            Err(e) => {
                                let error_msg = format!(
                                    "Failed to get commit info after merge of issue '{issue_name}': {e}"
                                );
                                tracing::error!("{}", error_msg);

                                // Create abort file to signal workflow termination
                                create_abort_file_current_dir(&error_msg);

                                return Err(McpError::invalid_params(error_msg, None));
                            }
                        };

                        // If delete_branch is true, delete the branch after successful merge
                        if request.delete_branch {
                            let branch_name = Self::format_issue_branch_name(&issue_name);
                            let branch_name_obj = swissarmyhammer_git::BranchName::new(
                                &branch_name,
                            )
                            .map_err(|e| {
                                McpError::internal_error(
                                    format!("Invalid branch name: {}", e),
                                    None,
                                )
                            })?;
                            match ops.delete_branch(&branch_name_obj) {
                                Ok(()) => {
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
                        let error_msg = format!("Merge failed for issue '{issue_name}': {e}");
                        tracing::error!("{}", error_msg);

                        // Create abort file to signal workflow termination
                        create_abort_file_current_dir(&error_msg);

                        Err(McpError::invalid_params(error_msg, None))
                    }
                }
            }
            None => {
                let error_msg = "Git operations not available for merge".to_string();
                tracing::error!("{}", error_msg);

                // Create abort file to signal workflow termination
                create_abort_file_current_dir(&error_msg);

                Err(McpError::invalid_params(error_msg, None))
            }
        }
    }
}
