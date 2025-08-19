//! Issue branch merging tool for MCP operations
//!
//! This module provides the MergeIssueTool for merging issue work branches.

use crate::mcp::responses::create_success_response;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::MergeIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use swissarmyhammer::common::create_abort_file_current_dir;
use crate::cli::CliExclusionMarker;

/// Tool for merging an issue work branch
///
/// This tool is designed for MCP workflow orchestration and should not be
/// exposed as a CLI command since it requires coordinated state between
/// git operations and issue storage, and uses MCP-specific abort patterns.
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct MergeIssueTool;

impl CliExclusionMarker for MergeIssueTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow orchestration tool - requires coordinated state management and uses abort file patterns")
    }
}

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
                match ops.current_branch() {
                    Ok(current_branch) => {
                        if !current_branch.starts_with("issue/") {
                            let abort_reason = format!(
                                "Cannot merge issue '{}' from branch '{}'. Merge operations must be performed from an issue branch. Switch to the appropriate issue branch first.",
                                request.name, current_branch
                            );

                            tracing::warn!("Invalid branch for merge operation: {}", abort_reason);

                            // Create abort file to signal workflow termination
                            create_abort_file_current_dir(&abort_reason);

                            return Err(McpError::invalid_params(abort_reason, None));
                        }
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
                // Merge the branch back using git merge-base to determine target
                match ops.merge_issue_branch_auto(&issue_name) {
                    Ok(target_branch) => {
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
                            match ops.delete_branch(&branch_name, false) {
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

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}
