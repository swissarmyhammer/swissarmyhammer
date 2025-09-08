//! Issue workflow management tool for MCP operations
//!
//! This module provides the WorkIssueTool for switching to work on a specific issue.

use crate::mcp::responses::create_success_response;
use crate::mcp::shared_utils::McpErrorHandler;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::WorkIssueRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::create_abort_file_current_dir;

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
            Some(ops) => match ops.get_current_branch() {
                Ok(Some(branch)) => branch.to_string(),
                Ok(None) => "HEAD".to_string(), // Handle detached HEAD
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(
                        e.into(),
                        "get current branch",
                    ))
                }
            },
            None => {
                return Err(McpError::internal_error(
                    "Git operations not available".to_string(),
                    None,
                ))
            }
        };

        // Enhanced validation for working on issues from other issue branches
        // First check if we're already on the correct issue branch - if so, no need to abort
        let target_branch = format!("issue/{}", request.name.0);
        if current_branch.starts_with("issue/") && current_branch != target_branch {
            let abort_reason = format!(
                "Cannot work on issue '{}' from issue branch '{}'. Issue branches cannot be used as source branches to prevent circular dependencies. Switch to a non-issue branch (such as a feature, develop, or base branch) first.",
                request.name.0, current_branch
            );

            // Create abort file to signal workflow termination
            create_abort_file_current_dir(&abort_reason);

            return Err(McpError::invalid_params(abort_reason, None));
        }

        let issue_storage = context.issue_storage.read().await;
        let issue_exists = issue_storage.get_issue(request.name.as_str()).await.is_ok();

        if issue_exists {
            // Issue exists - get it and use its stored source branch
            let issue = match issue_storage.get_issue(request.name.as_str()).await {
                Ok(issue) => issue,
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(
                        swissarmyhammer::SwissArmyHammerError::Other(e.to_string()),
                        "get issue",
                    ))
                }
            };

            let branch_name = issue.name.clone();

            match git_ops.as_mut() {
                Some(ops) => {
                    let full_branch_name = format!("issue/{}", branch_name);
                    let branch_name_obj = swissarmyhammer_git::BranchName::new(&full_branch_name)
                        .map_err(|e| {
                        McpError::internal_error(format!("Invalid branch name: {}", e), None)
                    })?;

                    // Check if branch exists first
                    let branch_exists = ops.branch_exists(&branch_name_obj).map_err(|e| {
                        McpErrorHandler::handle_error(e.into(), "check branch exists")
                    })?;

                    if branch_exists {
                        // Branch exists, just checkout
                        match ops.checkout_branch(&branch_name_obj) {
                            Ok(()) => Ok(create_success_response(format!(
                                "Switched to work branch: {full_branch_name}"
                            ))),
                            Err(e) => Err(McpErrorHandler::handle_error(
                                e.into(),
                                "checkout work branch",
                            )),
                        }
                    } else {
                        // Branch doesn't exist, create and checkout
                        match ops.create_and_checkout_branch(&branch_name_obj) {
                            Ok(()) => Ok(create_success_response(format!(
                                "Created and switched to work branch: {full_branch_name}"
                            ))),
                            Err(e) => Err(McpErrorHandler::handle_error(
                                e.into(),
                                "create work branch",
                            )),
                        }
                    }
                }
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

#[cfg(test)]
mod tests {
    use swissarmyhammer::test_utils::IsolatedTestHome;
    use swissarmyhammer_common::create_abort_file;
    use tempfile::TempDir;

    #[test]
    fn test_create_abort_file() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let reason = "Test abort reason for issue branching";
        create_abort_file(temp_path, reason).unwrap();

        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, reason);
    }
}
