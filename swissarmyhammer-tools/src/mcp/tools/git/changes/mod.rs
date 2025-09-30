//! Git changes tool - list files changed on a branch
//!
//! This tool provides programmatic access to git diff operations using libgit2,
//! identifying which files have been modified on a branch relative to its parent.
//!
//! ## Key Concept
//!
//! The tool determines the "scope of changes" for any branch:
//! - **Feature/Issue branches**: Files changed since diverging from the parent branch
//! - **Main/trunk branches**: All tracked files (cumulative changes)
//!
//! The distinction is based on whether a branch has a clear parent it diverged from.

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use swissarmyhammer_git::{GitOperations, GitResult};

use crate::mcp::tool_registry::{McpTool, ToolContext};

/// Request structure for git changes operation
#[derive(Debug, Deserialize, Serialize)]
pub struct GitChangesRequest {
    /// Branch name to analyze
    pub branch: String,
}

/// Response structure containing changed files
#[derive(Debug, Deserialize, Serialize)]
pub struct GitChangesResponse {
    /// The analyzed branch
    pub branch: String,
    /// Parent branch (if determined), null for root branches
    pub parent_branch: Option<String>,
    /// List of file paths that have changed
    pub files: Vec<String>,
}

/// Get all uncommitted changes in the working directory
///
/// Returns a deduplicated list of file paths that have uncommitted changes,
/// including:
/// - Staged modifications, additions, and deletions
/// - Unstaged modifications and deletions
/// - Renamed files
/// - Untracked files
///
/// # Arguments
///
/// * `git_ops` - GitOperations instance for the repository
///
/// # Returns
///
/// A sorted, deduplicated list of file paths with uncommitted changes
pub fn get_uncommitted_changes(git_ops: &GitOperations) -> GitResult<Vec<String>> {
    let status = git_ops.get_status()?;

    // Get all changed files (staged, modified, deleted, renamed)
    let mut files = status.all_changed_files();

    // Add untracked files (not included in all_changed_files)
    files.extend(status.untracked.clone());

    // Deduplicate and sort for consistent output
    files.sort();
    files.dedup();

    Ok(files)
}

/// Tool for listing changed files on a git branch
#[derive(Default)]
pub struct GitChangesTool;

impl GitChangesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GitChangesTool {
    fn name(&self) -> &'static str {
        "git_changes"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch name to analyze"
                }
            },
            "required": ["branch"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        // Parse request
        let request: GitChangesRequest =
            serde_json::from_value(serde_json::Value::Object(arguments))
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;

        // Get git operations from context
        let git_ops_guard = context.git_ops.lock().await;
        let git_ops = git_ops_guard
            .as_ref()
            .ok_or_else(|| rmcp::ErrorData::internal_error("Git operations not available", None))?;

        // Try to find parent branch using find_merge_target_for_issue
        let parent_branch = if request.branch.starts_with("issue/") {
            // For issue branches, try to find the parent branch
            use swissarmyhammer_git::BranchName;
            let branch_name = BranchName::new(&request.branch).map_err(|e| {
                rmcp::ErrorData::invalid_params(format!("Invalid branch name: {}", e), None)
            })?;
            git_ops.find_merge_target_for_issue(&branch_name).ok()
        } else {
            None
        };

        // Get changed files based on whether we have a parent branch
        let mut files = if let Some(ref parent) = parent_branch {
            // Feature/issue branch: get files changed from parent
            git_ops
                .get_changed_files_from_parent(&request.branch, parent)
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to get changed files: {}", e),
                        None,
                    )
                })?
        } else {
            // Main/trunk branch: get all tracked files
            git_ops.get_all_tracked_files().map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to get tracked files: {}", e), None)
            })?
        };

        // Merge in uncommitted changes
        let uncommitted = get_uncommitted_changes(git_ops).map_err(|e| {
            rmcp::ErrorData::internal_error(
                format!("Failed to get uncommitted changes: {}", e),
                None,
            )
        })?;
        files.extend(uncommitted);

        // Deduplicate and sort for consistent output
        files.sort();
        files.dedup();

        // Build response
        let response = GitChangesResponse {
            branch: request.branch,
            parent_branch,
            files,
        };

        // Serialize to JSON and create response
        let response_json = serde_json::to_string_pretty(&response).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to serialize response: {}", e), None)
        })?;

        Ok(CallToolResult {
            content: vec![rmcp::model::Annotated::new(
                rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                    text: response_json,
                    meta: None,
                }),
                None,
            )],
            is_error: Some(false),
            structured_content: None,
            meta: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use swissarmyhammer_git::GitOperations;
    use tempfile::TempDir;

    /// Helper function to initialize a git repository with config
    fn setup_test_repo(repo_path: &Path) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    /// Helper function to create an initial commit
    fn create_initial_commit(repo_path: &Path, filename: &str, content: &str) {
        std::fs::write(repo_path.join(filename), content).unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_get_uncommitted_changes_clean_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "initial.txt", "initial content");

        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 0, "Clean repository should have no changes");
    }

    #[test]
    fn test_get_uncommitted_changes_staged_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "initial.txt", "initial");

        // Create and stage a new file
        std::fs::write(repo_path.join("staged.txt"), "staged content").unwrap();
        std::process::Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"staged.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_unstaged_modifications() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "file.txt", "original");

        // Modify the file without staging
        std::fs::write(repo_path.join("file.txt"), "modified").unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"file.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_untracked_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "initial.txt", "initial");

        // Create untracked file
        std::fs::write(repo_path.join("untracked.txt"), "untracked content").unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"untracked.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_mixed_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "existing.txt", "existing");

        // Create various types of changes
        std::fs::write(repo_path.join("untracked.txt"), "untracked").unwrap();
        std::fs::write(repo_path.join("existing.txt"), "modified").unwrap();
        std::fs::write(repo_path.join("staged.txt"), "staged").unwrap();
        std::process::Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 3);
        assert!(changes.contains(&"untracked.txt".to_string()));
        assert!(changes.contains(&"existing.txt".to_string()));
        assert!(changes.contains(&"staged.txt".to_string()));

        // Verify sorted order
        let mut expected = vec![
            "existing.txt".to_string(),
            "staged.txt".to_string(),
            "untracked.txt".to_string(),
        ];
        expected.sort();
        assert_eq!(changes, expected);
    }

    #[test]
    fn test_git_changes_tool_name() {
        let tool = GitChangesTool::new();
        assert_eq!(tool.name(), "git_changes");
    }

    #[test]
    fn test_git_changes_tool_description() {
        let tool = GitChangesTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("Git Changes"));
    }

    #[test]
    fn test_git_changes_tool_schema() {
        let tool = GitChangesTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let properties = schema
            .get("properties")
            .expect("schema should have properties");
        assert!(properties.get("branch").is_some());

        let required = schema
            .get("required")
            .expect("schema should have required fields");
        assert!(required.is_array());
        let required_array = required.as_array().expect("required should be an array");
        assert_eq!(required_array.len(), 1);
        assert_eq!(required_array[0], "branch");
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_main_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);

        // Create and commit files on main
        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("main"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Parse response to verify structure
        let response_text = match &response.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        };
        let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.parent_branch, None);
        assert_eq!(parsed.files.len(), 2);
        assert!(parsed.files.contains(&"file1.txt".to_string()));
        assert!(parsed.files.contains(&"file2.txt".to_string()));
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_issue_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "base.txt", "base content");

        // Create issue branch and add files
        std::process::Command::new("git")
            .args(["checkout", "-b", "issue/test-feature"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::fs::write(repo_path.join("feature1.txt"), "feature content").unwrap();
        std::fs::write(repo_path.join("feature2.txt"), "more features").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Add features"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "branch".to_string(),
            serde_json::json!("issue/test-feature"),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Parse response
        let response_text = match &response.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        };
        let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();
        assert_eq!(parsed.branch, "issue/test-feature");
        assert_eq!(parsed.parent_branch, Some("main".to_string()));
        assert_eq!(parsed.files.len(), 2);
        assert!(parsed.files.contains(&"feature1.txt".to_string()));
        assert!(parsed.files.contains(&"feature2.txt".to_string()));
        // Base file should not be included
        assert!(!parsed.files.contains(&"base.txt".to_string()));
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_no_git_ops() {
        // Create test context without git ops
        let context = crate::test_utils::create_test_context().await;

        // Execute tool - should fail because git ops not available
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("main"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("not available"));
    }

    #[test]
    fn test_git_changes_request_serialization() {
        let request = GitChangesRequest {
            branch: "main".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("main"));

        let deserialized: GitChangesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, "main");
    }

    #[test]
    fn test_git_changes_response_serialization() {
        let response = GitChangesResponse {
            branch: "issue/test".to_string(),
            parent_branch: Some("main".to_string()),
            files: vec!["src/main.rs".to_string(), "README.md".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("issue/test"));
        assert!(json.contains("main"));
        assert!(json.contains("src/main.rs"));

        let deserialized: GitChangesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, "issue/test");
        assert_eq!(deserialized.parent_branch, Some("main".to_string()));
        assert_eq!(deserialized.files.len(), 2);
    }

    #[tokio::test]
    async fn test_git_changes_tool_includes_uncommitted_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);
        create_initial_commit(repo_path, "committed.txt", "committed content");

        // Create issue branch and add a committed file
        std::process::Command::new("git")
            .args(["checkout", "-b", "issue/test-uncommitted"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::fs::write(repo_path.join("committed_on_branch.txt"), "committed").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Add committed file"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create an uncommitted file
        std::fs::write(repo_path.join("uncommitted.txt"), "uncommitted content").unwrap();

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "branch".to_string(),
            serde_json::json!("issue/test-uncommitted"),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Parse response
        let response_text = match &response.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        };
        let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();

        // Verify both committed and uncommitted files are included
        assert_eq!(parsed.branch, "issue/test-uncommitted");
        assert_eq!(parsed.parent_branch, Some("main".to_string()));
        assert_eq!(parsed.files.len(), 2);
        assert!(parsed
            .files
            .contains(&"committed_on_branch.txt".to_string()));
        assert!(parsed.files.contains(&"uncommitted.txt".to_string()));

        // Base file should not be included
        assert!(!parsed.files.contains(&"committed.txt".to_string()));
    }

    #[tokio::test]
    async fn test_git_changes_tool_main_branch_includes_uncommitted() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        setup_test_repo(repo_path);

        // Create and commit files on main
        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create an uncommitted file
        std::fs::write(repo_path.join("uncommitted.txt"), "uncommitted").unwrap();

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo_path).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("main"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Parse response
        let response_text = match &response.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        };
        let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();

        // Verify all tracked files and uncommitted files are included
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.parent_branch, None);
        assert_eq!(parsed.files.len(), 3);
        assert!(parsed.files.contains(&"file1.txt".to_string()));
        assert!(parsed.files.contains(&"file2.txt".to_string()));
        assert!(parsed.files.contains(&"uncommitted.txt".to_string()));
    }
}
