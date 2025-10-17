//! Issue show tool for MCP operations
//!
//! This module provides the ShowIssueTool for displaying specific issues through the MCP protocol.
//!
//! # Overview
//!
//! The ShowIssueTool allows users to retrieve and display issue details by name or using special
//! parameters. It supports both formatted output (with metadata) and raw content output.
//!
//! # Special Parameters
//!
//! In addition to regular issue names, the tool supports special parameters:
//!
//! - `next`: Returns the next pending (active) issue alphabetically. If no pending issues exist,
//!   returns a message indicating all issues are completed.
//!
//! # Usage Examples
//!
//! Show a specific issue by name:
//! ```json
//! {
//!   "name": "FEATURE_000123_user-auth"
//! }
//! ```
//!
//! Show the next pending issue:
//! ```json
//! {
//!   "name": "next"
//! }
//! ```
//!
//! Show raw content without formatting:
//! ```json
//! {
//!   "name": "FEATURE_000123_user-auth",
//!   "raw": true
//! }
//! ```
//!
//! # Output Format
//!
//! By default, the tool returns formatted output including:
//! - Issue status (Active or Completed)
//! - Issue name
//! - File path
//! - Creation timestamp
//! - Full issue content
//!
//! When `raw: true` is specified, only the issue content is returned without metadata.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use swissarmyhammer_issues::IssueInfo;

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

    /// Format issue status as text
    ///
    /// Returns a formatted string indicating the issue status.
    /// Completed issues show "Completed", active issues show "Active".
    ///
    /// # Arguments
    ///
    /// * `completed` - Whether the issue is completed
    ///
    /// # Returns
    ///
    /// A static string containing the status text
    fn format_issue_status(completed: bool) -> &'static str {
        if completed {
            "Completed"
        } else {
            "Active"
        }
    }

    /// Format issue for display with metadata and content
    ///
    /// Creates a formatted display string including status, issue name,
    /// file path, creation date, and the full issue content.
    ///
    /// # Arguments
    ///
    /// * `issue_info` - The issue information to format
    ///
    /// # Returns
    ///
    /// A formatted string with all issue details
    fn format_issue_display(issue_info: &IssueInfo) -> String {
        let status = Self::format_issue_status(issue_info.completed);

        let mut result = format!("Status: {}\n", status);
        result.push_str(&format!("Issue: {}\n", issue_info.issue.name));
        result.push_str(&format!("File: {}\n", issue_info.file_path.display()));
        result.push_str(&format!(
            "Created: {}\n\n",
            issue_info.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
        result.push_str(&issue_info.issue.content);

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
                    "description": "Name of the issue to show. Use 'next' to show the next pending issue."
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

        // Validate issue name is not empty
        McpValidation::validate_not_empty(&request.name, "issue name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;

        tracing::debug!("Showing issue: {}", request.name);

        // Handle special parameters
        let issue_info = if request.name == "next" {
            // Get next pending issue and then get its info
            let issue_storage = context.issue_storage.read().await;
            match issue_storage.next_issue().await {
                Ok(Some(next_issue)) => {
                    // Get the full issue info for the next issue
                    match issue_storage.get_issue_info(&next_issue.name).await {
                        Ok(issue_info) => issue_info,
                        Err(e) => {
                            return Err(McpErrorHandler::handle_error(
                                swissarmyhammer_common::SwissArmyHammerError::Other {
                                    message: e.to_string(),
                                },
                                "get next issue info",
                            ));
                        }
                    }
                }
                Ok(None) => {
                    return Ok(BaseToolImpl::create_success_response(
                        "No pending issues found. All issues are completed!",
                    ));
                }
                Err(e) => {
                    return Err(McpErrorHandler::handle_error(
                        swissarmyhammer_common::SwissArmyHammerError::Other {
                            message: e.to_string(),
                        },
                        "get next issue",
                    ));
                }
            }
        } else {
            // Regular issue name lookup - get the issue with extended info
            let issue_storage = context.issue_storage.read().await;
            match issue_storage.get_issue_info(&request.name).await {
                Ok(issue_info) => issue_info,
                Err(_) => {
                    return Err(McpError::invalid_params(
                        format!("Issue '{}' not found", request.name),
                        None,
                    ));
                }
            }
        };

        let response = if request.raw.unwrap_or(false) {
            issue_info.issue.content
        } else {
            Self::format_issue_display(&issue_info)
        };

        tracing::info!("Showed issue {}", issue_info.issue.name);
        Ok(BaseToolImpl::create_success_response(&response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_handlers::ToolHandlers;
    use std::sync::Arc;
    use swissarmyhammer_config::agent::AgentConfig;
    use swissarmyhammer_git::GitOperations;
    use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
    use swissarmyhammer_memoranda::MarkdownMemoStorage;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    /// Guard that restores the current directory when dropped
    struct DirGuard {
        original: std::path::PathBuf,
    }

    impl DirGuard {
        fn new(new_dir: &std::path::Path) -> std::io::Result<Self> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(new_dir)?;
            Ok(Self { original })
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Helper to create a test context with a temporary directory
    async fn create_test_context(temp_dir: &TempDir) -> ToolContext {
        let issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
        std::fs::create_dir_all(&issues_dir).unwrap();

        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            FileSystemIssueStorage::new(issues_dir).unwrap(),
        )));

        let git_ops = GitOperations::new().ok();

        let memo_dir = temp_dir.path().join(".swissarmyhammer").join("memos");
        std::fs::create_dir_all(&memo_dir).unwrap();
        let memo_storage = Arc::new(RwLock::new(Box::new(MarkdownMemoStorage::new(memo_dir))
            as Box<dyn swissarmyhammer_memoranda::MemoStorage>));

        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
        let agent_config = Arc::new(AgentConfig::default());

        ToolContext {
            issue_storage,
            git_ops: Arc::new(tokio::sync::Mutex::new(git_ops)),
            memo_storage,
            tool_handlers,
            agent_config,
            notification_sender: None,
            progress_sender: None,
            mcp_server_port: Arc::new(RwLock::new(None)),
        }
    }

    /// Helper to create a test issue
    async fn create_test_issue(storage: &dyn IssueStorage, name: &str, content: &str) {
        storage
            .create_issue(name.to_string(), content.to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_show_issue_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = DirGuard::new(temp_dir.path()).unwrap();
        let context = create_test_context(&temp_dir).await;

        // Create a test issue
        {
            let issue_storage = context.issue_storage.read().await;
            create_test_issue(
                &**issue_storage,
                "specific_issue",
                "# Specific Issue\n\nDirect lookup",
            )
            .await;
        }

        // Execute the tool with specific issue name
        let tool = ShowIssueTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("specific_issue".to_string()),
        );

        let result = tool.execute(args, &context).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Verify the response contains the specific issue
        if let Some(content) = response.content.first() {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content.text.contains("specific_issue"),
                    "Response should contain the specific issue name"
                );
                assert!(
                    text_content.text.contains("Direct lookup"),
                    "Response should contain the issue content"
                );
            }
        }
    }
}
