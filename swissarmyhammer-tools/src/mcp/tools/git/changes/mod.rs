//! Git changes tool - list files changed on a branch
//!
//! This tool provides programmatic access to git diff operations using libgit2,
//! identifying which files have been modified on a branch relative to its parent.
//!
//! ## Key Concept
//!
//! The tool determines the "scope of changes" for any branch:
//! - **Feature/Issue branches**: Files changed since diverging from the parent branch (plus uncommitted changes)
//! - **Main/trunk branches**: Only uncommitted changes (staged + unstaged + untracked)
//!
//! The distinction is based on whether a branch has a clear parent it diverged from.

use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use swissarmyhammer_git::{GitOperations, GitResult};
use swissarmyhammer_operations::{
    generate_mcp_schema, Operation, ParamMeta, ParamType, SchemaConfig,
};

use crate::mcp::tool_registry::{McpTool, ToolContext};

/// Request structure for git changes operation
#[derive(Debug, Deserialize, Serialize)]
pub struct GitChangesRequest {
    /// Branch name to analyze (optional, defaults to current branch)
    pub branch: Option<String>,
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

/// Operation metadata for getting changed files
#[derive(Debug, Default)]
pub struct GetChanges;

static GET_CHANGES_PARAMS: &[ParamMeta] = &[ParamMeta::new("branch")
    .description("Branch name to analyze (optional, defaults to current branch)")
    .param_type(ParamType::String)];

impl Operation for GetChanges {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "changes"
    }
    fn description(&self) -> &'static str {
        "List files changed on a branch relative to its parent"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_CHANGES_PARAMS
    }
}

// Static operation instances for schema generation
static GET_CHANGES: Lazy<GetChanges> = Lazy::new(GetChanges::default);

pub static GIT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> =
    Lazy::new(|| vec![&*GET_CHANGES as &dyn Operation]);

/// Tool for listing changed files on a git branch
#[derive(Default)]
pub struct GitChangesTool;

impl GitChangesTool {
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(GitChangesTool);

#[async_trait]
impl McpTool for GitChangesTool {
    fn name(&self) -> &'static str {
        "git"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let config = SchemaConfig::new(
            "Git operations for analyzing branch changes. Lists files changed on a branch relative to its parent, including uncommitted changes.",
        );
        generate_mcp_schema(&GIT_OPERATIONS, config)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &GIT_OPERATIONS;
        // SAFETY: GIT_OPERATIONS is a static Lazy<Vec<...>> initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip op from arguments before parsing
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "get changes" | "" => {
                // Default: get changes (only operation)
            }
            other => {
                return Err(rmcp::ErrorData::invalid_params(
                    format!(
                        "Unknown operation '{}'. Valid operations: 'get changes'",
                        other
                    ),
                    None,
                ));
            }
        }

        // Parse request
        let request: GitChangesRequest = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;

        // Get git operations from context
        let git_ops_guard = context.git_ops.lock().await;
        let git_ops = git_ops_guard
            .as_ref()
            .ok_or_else(|| rmcp::ErrorData::internal_error("Git operations not available", None))?;

        // Resolve branch name: use provided value or default to current branch
        let branch = match request.branch {
            Some(b) => b,
            None => git_ops.current_branch().map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed to get current branch: {}", e),
                    None,
                )
            })?,
        };

        // Try to find parent branch for any branch
        let parent_branch = {
            use swissarmyhammer_git::BranchName;
            let branch_name = BranchName::new(&branch).map_err(|e| {
                rmcp::ErrorData::invalid_params(format!("Invalid branch name: {}", e), None)
            })?;

            // Try to find merge target, but if it returns the branch itself or fails, treat as no parent
            match git_ops.find_merge_target_for_issue(&branch_name) {
                Ok(target) if target != branch => Some(target),
                _ => None,
            }
        };

        // Get changed files based on whether we have a parent branch
        let mut files = if let Some(ref parent) = parent_branch {
            // Feature/issue branch: get files changed from parent
            git_ops
                .get_changed_files_from_parent(&branch, parent)
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to get changed files: {}", e),
                        None,
                    )
                })?
        } else {
            // Main/trunk branch: get only uncommitted changes
            get_uncommitted_changes(git_ops).map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed to get uncommitted changes: {}", e),
                    None,
                )
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
            branch,
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
    use crate::test_utils::git_test_helpers::TestGitRepo;
    use swissarmyhammer_git::GitOperations;

    #[test]
    fn test_git_tool_has_operations() {
        let tool = GitChangesTool::new();
        let ops = tool.operations();
        assert_eq!(ops.len(), 1);
        assert!(ops.iter().any(|o| o.op_string() == "get changes"));
    }

    #[test]
    fn test_get_uncommitted_changes_clean_repo() {
        let repo = TestGitRepo::new();
        repo.commit_file("initial.txt", "initial content", "Initial commit");

        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 0, "Clean repository should have no changes");
    }

    #[test]
    fn test_get_uncommitted_changes_staged_files() {
        let repo = TestGitRepo::new();
        repo.commit_file("initial.txt", "initial", "Initial commit");

        // Create and stage a new file
        repo.create_file("staged.txt", "staged content");
        repo.add_all();

        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"staged.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_unstaged_modifications() {
        let repo = TestGitRepo::new();
        repo.commit_file("file.txt", "original", "Initial commit");

        // Modify the file without staging
        repo.create_file("file.txt", "modified");

        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"file.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_untracked_files() {
        let repo = TestGitRepo::new();
        repo.commit_file("initial.txt", "initial", "Initial commit");

        // Create untracked file
        repo.create_file("untracked.txt", "untracked content");

        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let changes = get_uncommitted_changes(&git_ops).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes.contains(&"untracked.txt".to_string()));
    }

    #[test]
    fn test_get_uncommitted_changes_mixed_changes() {
        let repo = TestGitRepo::new();
        repo.commit_file("existing.txt", "existing", "Initial commit");

        // Create various types of changes
        repo.create_file("untracked.txt", "untracked");
        repo.create_file("existing.txt", "modified");
        repo.create_file("staged.txt", "staged");
        repo.add_all();

        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
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
        assert_eq!(tool.name(), "git");
    }

    #[test]
    fn test_git_changes_tool_description() {
        let tool = GitChangesTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("List files that have changed"));
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
        assert!(properties.get("op").is_some());

        // Should have operation schemas
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_main_branch() {
        let repo = TestGitRepo::new();

        // Create and commit files on main
        repo.create_file("file1.txt", "content1");
        repo.create_file("file2.txt", "content2");
        repo.add_all();
        repo.commit("Initial commit");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
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
        // Main branch with no uncommitted changes should return empty list
        assert_eq!(parsed.files.len(), 0);
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_default_branch() {
        let repo = TestGitRepo::new();

        // Create and commit files on main
        repo.create_file("file1.txt", "content1");
        repo.add_all();
        repo.commit("Initial commit");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool with no branch specified - should use current branch
        let tool = GitChangesTool::new();
        let arguments = serde_json::Map::new(); // Empty arguments

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
        // Should use current branch (main)
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.parent_branch, None);
    }

    #[tokio::test]
    async fn test_git_changes_tool_execute_issue_branch() {
        let repo = TestGitRepo::new();
        repo.commit_file("base.txt", "base content", "Initial commit");

        // Create issue branch and add files
        repo.create_and_checkout_branch("issue/test-feature");
        repo.create_file("feature1.txt", "feature content");
        repo.create_file("feature2.txt", "more features");
        repo.add_all();
        repo.commit("Add features");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
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
            branch: Some("main".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("main"));

        let deserialized: GitChangesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, Some("main".to_string()));

        // Test with no branch specified
        let empty_request: GitChangesRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(empty_request.branch, None);
    }

    #[test]
    fn test_git_changes_response_serialization() {
        let response = GitChangesResponse {
            branch: "test-branch".to_string(),
            parent_branch: Some("main".to_string()),
            files: vec!["src/main.rs".to_string(), "README.md".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test-branch"));
        assert!(json.contains("main"));
        assert!(json.contains("src/main.rs"));

        let deserialized: GitChangesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.branch, "test-branch");
        assert_eq!(deserialized.parent_branch, Some("main".to_string()));
        assert_eq!(deserialized.files.len(), 2);
    }

    #[tokio::test]
    async fn test_git_changes_tool_includes_uncommitted_changes() {
        let repo = TestGitRepo::new();
        repo.commit_file("committed.txt", "committed content", "Initial commit");

        // Create branch and add a committed file
        repo.create_and_checkout_branch("test-uncommitted");
        repo.commit_file("committed_on_branch.txt", "committed", "Add committed file");

        // Create an uncommitted file
        repo.create_file("uncommitted.txt", "uncommitted content");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("test-uncommitted"));

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
        assert_eq!(parsed.branch, "test-uncommitted");
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
        let repo = TestGitRepo::new();

        // Create and commit files on main
        repo.create_file("file1.txt", "content1");
        repo.create_file("file2.txt", "content2");
        repo.add_all();
        repo.commit("Initial commit");

        // Create an uncommitted file
        repo.create_file("uncommitted.txt", "uncommitted");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
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

        // Main branch should only return uncommitted files, not all tracked files
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.parent_branch, None);
        assert_eq!(parsed.files.len(), 1);
        assert!(parsed.files.contains(&"uncommitted.txt".to_string()));
        // Committed files should NOT be included
        assert!(!parsed.files.contains(&"file1.txt".to_string()));
        assert!(!parsed.files.contains(&"file2.txt".to_string()));
    }

    #[tokio::test]
    async fn test_git_changes_tool_invalid_branch() {
        let repo = TestGitRepo::new();
        repo.commit_file("initial.txt", "initial content", "Initial commit");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool with non-existent branch
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "branch".to_string(),
            serde_json::json!("non-existent-branch"),
        );

        let result = tool.execute(arguments, &context).await;

        // For non-existent branches, the tool falls back to get_uncommitted_changes
        // which returns only uncommitted files. This is acceptable behavior.
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        let response_text = match &response.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("Expected text content"),
        };
        let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();
        assert_eq!(parsed.branch, "non-existent-branch");
        // Should have no parent since it doesn't exist
        assert_eq!(parsed.parent_branch, None);
        // Should have no files since there are no uncommitted changes
        assert_eq!(parsed.files.len(), 0);
    }

    #[tokio::test]
    async fn test_git_changes_tool_non_git_directory() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path();

        // Create a regular directory without initializing git
        std::fs::write(non_git_path.join("regular_file.txt"), "content").unwrap();

        // Try to create git ops - should fail
        let git_ops_result = GitOperations::with_work_dir(non_git_path);
        assert!(git_ops_result.is_err());
    }

    #[tokio::test]
    async fn test_git_changes_tool_empty_repository() {
        let repo = TestGitRepo::new();

        // Create test context with git ops (empty repo, no commits yet)
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool on main branch in empty repo
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert("branch".to_string(), serde_json::json!("main"));

        let result = tool.execute(arguments, &context).await;

        // Empty repository should either succeed with empty files or fail gracefully
        match result {
            Ok(response) => {
                let response_text = match &response.content[0].raw {
                    rmcp::model::RawContent::Text(text) => &text.text,
                    _ => panic!("Expected text content"),
                };
                let parsed: GitChangesResponse = serde_json::from_str(response_text).unwrap();
                // Empty repo should have no tracked files
                assert_eq!(parsed.files.len(), 0);
            }
            Err(error) => {
                // Or it might fail with an appropriate error
                assert!(
                    error.message.contains("Failed to get")
                        || error.message.contains("empty")
                        || error.message.contains("no commits")
                );
            }
        }
    }

    // Note: Orphan branch test requires shell commands for git checkout --orphan
    // which libgit2 doesn't directly support. This edge case is left with shell commands.
    // In production code, orphan branches are extremely rare and handled correctly by
    // the parent branch detection logic (no parent = treat as main branch).

    #[tokio::test]
    async fn test_git_changes_tool_feature_branch_detects_parent() {
        let repo = TestGitRepo::new();
        repo.commit_file("base.txt", "base content", "Initial commit");

        // Create feature branch and add files
        repo.create_and_checkout_branch("feature/new-feature");
        repo.create_file("feature1.txt", "feature content");
        repo.create_file("feature2.txt", "more features");
        repo.add_all();
        repo.commit("Add features");

        // Create test context with git ops
        let git_ops = GitOperations::with_work_dir(repo.path()).unwrap();
        let context = crate::test_utils::create_test_context().await;
        *context.git_ops.lock().await = Some(git_ops);

        // Execute tool
        let tool = GitChangesTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "branch".to_string(),
            serde_json::json!("feature/new-feature"),
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

        // Feature branch should detect main as parent and show only changed files
        assert_eq!(parsed.branch, "feature/new-feature");
        assert_eq!(parsed.parent_branch, Some("main".to_string()));
        assert_eq!(parsed.files.len(), 2);
        assert!(parsed.files.contains(&"feature1.txt".to_string()));
        assert!(parsed.files.contains(&"feature2.txt".to_string()));
        // Base file should not be included
        assert!(!parsed.files.contains(&"base.txt".to_string()));
    }
}
