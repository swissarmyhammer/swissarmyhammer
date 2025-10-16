//! Issue show tool for MCP operations
//!
//! This module provides the ShowIssueTool for displaying specific issues through the MCP protocol.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use swissarmyhammer_common::SwissArmyHammerError;
use swissarmyhammer_issues::Config;
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

    /// Format issue status as colored emoji
    fn format_issue_status(completed: bool) -> &'static str {
        if completed {
            "✅ Completed"
        } else {
            "🔄 Active"
        }
    }

    /// Format issue for display
    fn format_issue_display(issue_info: &IssueInfo) -> String {
        let status = Self::format_issue_status(issue_info.completed);

        let mut result = format!("{} Issue: {}\n", status, issue_info.issue.name);
        result.push_str(&format!("📁 File: {}\n", issue_info.file_path.display()));
        result.push_str(&format!(
            "📅 Created: {}\n\n",
            issue_info.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
        result.push_str(&issue_info.issue.content);

        result
    }

    /// Get issue name from git branch (fallback when marker doesn't exist)
    /// Returns either Ok(issue_name) or Err with either a CallToolResult (success response)
    /// or propagates the McpError
    async fn get_issue_name_from_branch(
        git_ops: &tokio::sync::Mutex<Option<swissarmyhammer_git::GitOperations>>,
    ) -> Result<String, Result<CallToolResult, McpError>> {
        let git_ops_guard = git_ops.lock().await;
        match git_ops_guard.as_ref() {
            Some(ops) => match ops.get_current_branch() {
                Ok(Some(branch)) => {
                    let branch_str = branch.to_string();
                    let config = Config::global();
                    if let Some(issue_name) = branch_str.strip_prefix(&config.issue_branch_prefix) {
                        Ok(issue_name.to_string())
                    } else {
                        Err(Ok(BaseToolImpl::create_success_response(format!(
                            "Not on an issue branch and no current issue marker set. Current branch: {branch_str}"
                        ))))
                    }
                }
                Ok(None) => Err(Ok(BaseToolImpl::create_success_response(
                    "Not on any branch (detached HEAD) and no current issue marker set".to_string(),
                ))),
                Err(e) => Err(Err(McpErrorHandler::handle_error(
                    SwissArmyHammerError::Other {
                        message: e.to_string(),
                    },
                    "get current branch",
                ))),
            },
            None => Err(Ok(BaseToolImpl::create_success_response(
                "Git operations not available and no current issue marker set",
            ))),
        }
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

        // Validate issue name is not empty
        McpValidation::validate_not_empty(&request.name, "issue name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;

        tracing::debug!("Showing issue: {}", request.name);

        // Handle special parameters
        let issue_info = if request.name == "current" {
            // Precedence order: marker file > git branch > error
            // 1. Try marker file first (primary method)
            // 2. Fall back to git branch detection (compatibility/backward compatibility)
            // 3. Return error if neither method works
            let issue_name = match swissarmyhammer_issues::current_marker::get_current_issue() {
                Ok(Some(name)) => {
                    // Marker file exists and has a value - use it (highest precedence)
                    tracing::debug!("Found current issue from marker: {}", name);
                    name
                }
                Ok(None) => {
                    // Marker doesn't exist - fall back to git branch detection for backward compatibility
                    tracing::debug!("No marker file found, falling back to git branch detection");
                    match Self::get_issue_name_from_branch(&context.git_ops).await {
                        Ok(name) => name,
                        Err(Ok(result)) => return Ok(result),
                        Err(Err(error)) => return Err(error),
                    }
                }
                Err(e) => {
                    // Marker read failed (I/O error, permission, etc.) - fall back gracefully to git branch
                    tracing::warn!(
                        "Failed to read current issue marker, using git branch fallback: {}",
                        e
                    );
                    match Self::get_issue_name_from_branch(&context.git_ops).await {
                        Ok(name) => name,
                        Err(Ok(result)) => return Ok(result),
                        Err(Err(error)) => return Err(error),
                    }
                }
            };

            // Find the current issue in storage
            let issue_storage = context.issue_storage.read().await;
            match issue_storage.get_issue_info(&issue_name).await {
                Ok(issue_info) => issue_info,
                Err(_) => {
                    return Err(McpError::invalid_params(
                        format!("Issue '{issue_name}' not found"),
                        None,
                    ));
                }
            }
        } else if request.name == "next" {
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
        }
    }

    /// Helper to create a test issue
    async fn create_test_issue(storage: &dyn IssueStorage, name: &str, content: &str) {
        storage
            .create_issue(name.to_string(), content.to_string())
            .await
            .unwrap();
    }

    /// Helper to setup a git repository for testing
    fn setup_git_repo(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()?;

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()?;

        std::fs::write(path.join("test.txt"), "test")?;
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(path)
            .output()?;

        Ok(())
    }

    #[tokio::test]
    async fn test_show_issue_from_marker() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = DirGuard::new(temp_dir.path()).unwrap();

        let context = create_test_context(&temp_dir).await;

        // Create a test issue
        {
            let issue_storage = context.issue_storage.read().await;
            create_test_issue(
                &**issue_storage,
                "test_issue_marker",
                "# Test Issue\n\nContent from marker",
            )
            .await;
        }

        // Set the marker to this issue (uses current directory)
        swissarmyhammer_issues::current_marker::set_current_issue("test_issue_marker").unwrap();

        // Execute the tool with "current"
        let tool = ShowIssueTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("current".to_string()),
        );

        let result = tool.execute(args, &context).await;

        if let Err(e) = &result {
            panic!("Tool execution failed: {:?}", e);
        }
        let response = result.unwrap();

        // Verify the response contains the issue name
        if let Some(content) = response.content.first() {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content.text.contains("test_issue_marker"),
                    "Response should contain issue name from marker"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_show_issue_fallback_to_branch() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = DirGuard::new(temp_dir.path()).unwrap();

        // Initialize git repo
        setup_git_repo(temp_dir.path()).unwrap();

        // Create and checkout an issue branch
        std::process::Command::new("git")
            .args(["checkout", "-b", "issue/test_branch_issue"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let context = create_test_context(&temp_dir).await;

        // Create a test issue
        {
            let issue_storage = context.issue_storage.read().await;
            create_test_issue(
                &**issue_storage,
                "test_branch_issue",
                "# Test Issue\n\nContent from branch",
            )
            .await;
        }

        // Do NOT set marker - should fall back to branch

        // Execute the tool with "current"
        let tool = ShowIssueTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("current".to_string()),
        );

        let result = tool.execute(args, &context).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Verify the response contains the issue name from branch
        if let Some(content) = response.content.first() {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content.text.contains("test_branch_issue"),
                    "Response should contain issue name from branch"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_show_issue_marker_takes_precedence() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = DirGuard::new(temp_dir.path()).unwrap();

        // Initialize git repo
        setup_git_repo(temp_dir.path()).unwrap();

        // Create and checkout an issue branch
        std::process::Command::new("git")
            .args(["checkout", "-b", "issue/branch_issue"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let context = create_test_context(&temp_dir).await;

        // Create TWO test issues - one for branch, one for marker
        {
            let issue_storage = context.issue_storage.read().await;
            create_test_issue(
                &**issue_storage,
                "branch_issue",
                "# Branch Issue\n\nFrom branch",
            )
            .await;
            create_test_issue(
                &**issue_storage,
                "marker_issue",
                "# Marker Issue\n\nFrom marker",
            )
            .await;
        }

        // Set marker to a DIFFERENT issue than the branch
        swissarmyhammer_issues::current_marker::set_current_issue("marker_issue").unwrap();

        // Execute the tool with "current"
        let tool = ShowIssueTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("current".to_string()),
        );

        let result = tool.execute(args, &context).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Verify the response contains the marker issue, NOT the branch issue
        if let Some(content) = response.content.first() {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content.text.contains("marker_issue"),
                    "Response should contain issue name from marker (marker takes precedence)"
                );
                assert!(
                    !text_content.text.contains("branch_issue"),
                    "Response should NOT contain issue name from branch when marker exists"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_show_issue_no_marker_no_branch() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = DirGuard::new(temp_dir.path()).unwrap();

        // Initialize git repo (stays on main branch - not an issue branch)
        setup_git_repo(temp_dir.path()).unwrap();

        let context = create_test_context(&temp_dir).await;

        // Do NOT set marker, do NOT create issue branch

        // Execute the tool with "current"
        let tool = ShowIssueTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("current".to_string()),
        );

        let result = tool.execute(args, &context).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // Verify the response indicates no current issue
        if let Some(content) = response.content.first() {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                assert!(
                    text_content
                        .text
                        .contains("Not on an issue branch and no current issue marker set"),
                    "Response should indicate neither marker nor branch available"
                );
            }
        }
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
