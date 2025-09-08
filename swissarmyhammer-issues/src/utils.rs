//! Shared utilities for issue management
//!
//! This module provides common utilities that can be used by both CLI and MCP implementations
//! to ensure consistent behavior and reduce code duplication.

use crate::error::{Error, Result};
use crate::storage::IssueStorage;
use crate::types::{Issue, IssueInfo};
use std::io::{self, Read};
use std::path::PathBuf;
use swissarmyhammer_git::GitOperations;

/// Content source for issue operations
#[derive(Debug, Clone, PartialEq)]
pub enum ContentSource {
    /// Direct string content
    Direct(String),
    /// Content from a file path
    File(PathBuf),
    /// Content from stdin (indicated by "-")
    Stdin,
    /// No content provided (empty)
    Empty,
}

impl ContentSource {
    /// Create a ContentSource from CLI-style arguments
    pub fn from_args(content: Option<String>, file: Option<PathBuf>) -> Result<Self> {
        match (content, file) {
            (Some(content), None) => {
                if content == "-" {
                    Ok(ContentSource::Stdin)
                } else {
                    Ok(ContentSource::Direct(content))
                }
            }
            (None, Some(path)) => Ok(ContentSource::File(path)),
            (Some(_), Some(_)) => Err(Error::other("Cannot specify both content and file options")),
            (None, None) => Ok(ContentSource::Empty),
        }
    }

    /// Read the content from the source
    pub fn read_content(&self) -> Result<String> {
        match self {
            ContentSource::Direct(content) => Ok(content.clone()),
            ContentSource::File(path) => {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| Error::other(format!("Failed to read file {path:?}: {e}")))?;
                Ok(content.trim().to_string())
            }
            ContentSource::Stdin => {
                let mut buffer = String::new();
                io::stdin()
                    .read_to_string(&mut buffer)
                    .map_err(|e| Error::other(format!("Failed to read from stdin: {e}")))?;
                Ok(buffer.trim().to_string())
            }
            ContentSource::Empty => Ok(String::new()),
        }
    }
}

/// Convenience function for getting content from CLI-style arguments
/// This maintains backward compatibility with existing CLI code
pub fn get_content_from_args(content: Option<String>, file: Option<PathBuf>) -> Result<String> {
    let source = ContentSource::from_args(content, file)?;
    source.read_content()
}

/// Result of issue branch operations
#[derive(Debug, Clone)]
pub struct IssueBranchResult {
    /// The issue that was operated on
    pub issue: Issue,
    /// The git branch name
    pub branch_name: String,
    /// Whether this was a new branch creation or existing branch checkout
    pub created_new_branch: bool,
}

/// Result of issue merge operations
#[derive(Debug, Clone)]
pub struct IssueMergeResult {
    /// The issue that was merged
    pub issue: Issue,
    /// The branch that was merged
    pub branch_name: String,
    /// Whether the branch was deleted after merge
    pub branch_deleted: bool,
}

/// Create or switch to a work branch for an issue
///
/// This function encapsulates the business logic for issue branch management:
/// - Validates the issue exists
/// - Creates or switches to the appropriate issue branch
/// - Handles the git operations consistently
pub async fn work_on_issue<S: IssueStorage>(
    issue_name: &str,
    storage: &S,
    git_ops: &GitOperations,
) -> Result<IssueBranchResult> {
    // Get the issue to ensure it exists
    let issue = storage.get_issue(issue_name).await?;

    // Create work branch with format: issue/{issue_name}
    let branch_name = format!("issue/{}", issue.name);
    let current_branch = git_ops
        .get_current_branch()?
        .map(|b| b.to_string())
        .unwrap_or_default();
    let created_new_branch = current_branch != branch_name;

    // Create or switch to the work branch (branch from current HEAD)
    let branch_name_obj = swissarmyhammer_git::BranchName::new(&branch_name)
        .map_err(|e| Error::other(format!("Invalid branch name: {}", e)))?;
    git_ops.create_and_checkout_branch(&branch_name_obj)?;
    let actual_branch_name = branch_name.clone();

    Ok(IssueBranchResult {
        issue,
        branch_name: actual_branch_name,
        created_new_branch,
    })
}

/// Get the current issue being worked on based on git branch
///
/// This function determines the current issue by parsing the git branch name
/// to extract the issue name from branches following the "issue/{name}" pattern.
pub fn get_current_issue_from_branch(git_ops: &GitOperations) -> Result<Option<String>> {
    let current_branch = git_ops
        .get_current_branch()?
        .map(|b| b.to_string())
        .unwrap_or_default();

    // Parse issue name from branch name pattern: issue/{issue_name}
    if let Some(stripped) = current_branch.strip_prefix("issue/") {
        Ok(Some(stripped.to_string()))
    } else {
        Ok(None)
    }
}

/// Project status and progress statistics
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    /// Total number of issues
    pub total_issues: usize,
    /// Number of completed issues
    pub completed_count: usize,
    /// Number of active (non-completed) issues
    pub active_count: usize,
    /// Completion percentage (0-100)
    pub completion_percentage: usize,
    /// Whether all issues are complete
    pub all_complete: bool,
    /// List of active issues
    pub active_issues: Vec<Issue>,
    /// List of completed issues
    pub completed_issues: Vec<Issue>,
}

impl ProjectStatus {
    /// Create a new project status from a list of issue infos
    pub fn from_issue_infos(issue_infos: Vec<IssueInfo>) -> Self {
        let completed_issues: Vec<Issue> = issue_infos
            .iter()
            .filter(|i| i.completed)
            .map(|i| i.issue.clone())
            .collect();
        let active_issues: Vec<Issue> = issue_infos
            .iter()
            .filter(|i| !i.completed)
            .map(|i| i.issue.clone())
            .collect();

        let total_issues = issue_infos.len();
        let completed_count = completed_issues.len();
        let active_count = active_issues.len();

        let completion_percentage = if total_issues > 0 {
            (completed_count * 100) / total_issues
        } else {
            0
        };

        let all_complete = active_count == 0 && total_issues > 0;

        Self {
            total_issues,
            completed_count,
            active_count,
            completion_percentage,
            all_complete,
            active_issues,
            completed_issues,
        }
    }

    /// Generate a simple status summary text
    pub fn summary(&self) -> String {
        format!(
            "Total: {}, Active: {}, Completed: {} ({}%)",
            self.total_issues, self.active_count, self.completed_count, self.completion_percentage
        )
    }

    /// Generate a detailed status report
    pub fn detailed_report(&self) -> String {
        if self.all_complete {
            format!(
                "🎉 All issues are complete!\n\n📊 Project Status:\n• Total Issues: {}\n• Completed: {} (100%)\n• Active: 0\n\n✅ Completed Issues:\n{}",
                self.total_issues,
                self.completed_count,
                self.completed_issues.iter()
                    .map(|issue| format!("• {}", issue.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            let active_list = self
                .active_issues
                .iter()
                .map(|issue| format!("• {}", issue.name))
                .collect::<Vec<_>>()
                .join("\n");

            let completed_list = if self.completed_count > 0 {
                self.completed_issues
                    .iter()
                    .map(|issue| format!("• {}", issue.name))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                "None".to_string()
            };

            format!(
                "⏳ Project has active issues ({}% complete)\n\n📊 Project Status:\n• Total Issues: {}\n• Completed: {} ({}%)\n• Active: {}\n\n🔄 Active Issues:\n{}\n\n✅ Completed Issues:\n{}",
                self.completion_percentage,
                self.total_issues,
                self.completed_count,
                self.completion_percentage,
                self.active_count,
                active_list,
                completed_list
            )
        }
    }
}

/// Get comprehensive project status
pub async fn get_project_status<S: IssueStorage>(storage: &S) -> Result<ProjectStatus> {
    let all_issue_infos = storage.list_issues_info().await?;
    Ok(ProjectStatus::from_issue_infos(all_issue_infos))
}

/// Format issue status for display
pub fn format_issue_status(completed: bool) -> String {
    if completed {
        "✅ Completed".to_string()
    } else {
        "🔄 Active".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_content_source_from_args() {
        // Direct content
        let source = ContentSource::from_args(Some("test content".to_string()), None).unwrap();
        assert_eq!(source, ContentSource::Direct("test content".to_string()));

        // Stdin indicator
        let source = ContentSource::from_args(Some("-".to_string()), None).unwrap();
        assert_eq!(source, ContentSource::Stdin);

        // File path
        let path = PathBuf::from("/test/path");
        let source = ContentSource::from_args(None, Some(path.clone())).unwrap();
        assert_eq!(source, ContentSource::File(path));

        // Empty
        let source = ContentSource::from_args(None, None).unwrap();
        assert_eq!(source, ContentSource::Empty);

        // Error case: both content and file
        let result =
            ContentSource::from_args(Some("content".to_string()), Some(PathBuf::from("/test")));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_content_direct() {
        let source = ContentSource::Direct("test content".to_string());
        assert_eq!(source.read_content().unwrap(), "test content");
    }

    #[test]
    fn test_read_content_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "file content\n").unwrap();

        let source = ContentSource::File(file_path);
        assert_eq!(source.read_content().unwrap(), "file content");
    }

    #[test]
    fn test_read_content_empty() {
        let source = ContentSource::Empty;
        assert_eq!(source.read_content().unwrap(), "");
    }

    #[test]
    fn test_get_content_from_args_convenience() {
        // Test direct content
        let result = get_content_from_args(Some("test".to_string()), None).unwrap();
        assert_eq!(result, "test");

        // Test empty
        let result = get_content_from_args(None, None).unwrap();
        assert_eq!(result, "");

        // Test file (would need actual file for full test)
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "file content\n").unwrap();

        let result = get_content_from_args(None, Some(file_path)).unwrap();
        assert_eq!(result, "file content");
    }

    #[test]
    fn test_get_current_issue_from_branch() {
        use swissarmyhammer_git::GitOperations;

        use tempfile::TempDir;

        // Create a temporary git repository for testing
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo using git2
        let repo = git2::Repository::init(repo_path).unwrap();

        // Set git config for the test
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit using git2
        std::fs::write(repo_path.join("README.md"), "Test").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();

        // Test main branch (should return None)
        let result = get_current_issue_from_branch(&git_ops).unwrap();
        assert_eq!(result, None);

        // Create and switch to issue branch using git2
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo
            .branch("issue/test_issue", &head_commit, false)
            .unwrap();
        repo.set_head(branch.get().name().unwrap()).unwrap();
        repo.checkout_head(None).unwrap();

        // Test issue branch (should return Some("test_issue"))
        let result = get_current_issue_from_branch(&git_ops).unwrap();
        assert_eq!(result, Some("test_issue".to_string()));

        // Test complex issue name using git2
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo
            .branch("issue/01K0S1158ADEHEQ28YMNBJHW97", &head_commit, false)
            .unwrap();
        repo.set_head(branch.get().name().unwrap()).unwrap();
        repo.checkout_head(None).unwrap();

        let result = get_current_issue_from_branch(&git_ops).unwrap();
        assert_eq!(result, Some("01K0S1158ADEHEQ28YMNBJHW97".to_string()));
    }

    #[test]
    fn test_project_status() {
        use chrono::Utc;
        use std::path::PathBuf;

        // Create test issues
        let active_issue1 = Issue {
            name: "active1".to_string(),
            content: "Active issue 1".to_string(),
        };

        let active_issue2 = Issue {
            name: "active2".to_string(),
            content: "Active issue 2".to_string(),
        };

        let completed_issue = Issue {
            name: "completed1".to_string(),
            content: "Completed issue".to_string(),
        };

        let issue_infos = vec![
            IssueInfo {
                issue: active_issue1.clone(),
                completed: false,
                file_path: PathBuf::from("/test/active1.md"),
                created_at: Utc::now(),
            },
            IssueInfo {
                issue: active_issue2.clone(),
                completed: false,
                file_path: PathBuf::from("/test/active2.md"),
                created_at: Utc::now(),
            },
            IssueInfo {
                issue: completed_issue.clone(),
                completed: true,
                file_path: PathBuf::from("/test/completed/completed1.md"),
                created_at: Utc::now(),
            },
        ];
        let status = ProjectStatus::from_issue_infos(issue_infos);

        // Check counts
        assert_eq!(status.total_issues, 3);
        assert_eq!(status.active_count, 2);
        assert_eq!(status.completed_count, 1);
        assert_eq!(status.completion_percentage, 33); // 1/3 = 33%
        assert!(!status.all_complete);

        // Check issue lists
        assert_eq!(status.active_issues.len(), 2);
        assert_eq!(status.completed_issues.len(), 1);
        assert_eq!(status.active_issues[0].name, "active1");
        assert_eq!(status.active_issues[1].name, "active2");
        assert_eq!(status.completed_issues[0].name, "completed1");

        // Check summary
        let summary = status.summary();
        assert_eq!(summary, "Total: 3, Active: 2, Completed: 1 (33%)");

        // Test all complete case
        let all_completed = vec![IssueInfo {
            issue: completed_issue.clone(),
            completed: true,
            file_path: PathBuf::from("/test/completed/completed1.md"),
            created_at: Utc::now(),
        }];
        let all_complete_status = ProjectStatus::from_issue_infos(all_completed);
        assert!(all_complete_status.all_complete);
        assert_eq!(all_complete_status.completion_percentage, 100);

        // Test empty case
        let empty_status = ProjectStatus::from_issue_infos(vec![]);
        assert_eq!(empty_status.total_issues, 0);
        assert_eq!(empty_status.completion_percentage, 0);
        assert!(!empty_status.all_complete); // Empty project is not "complete"
    }

    #[test]
    fn test_format_issue_status() {
        assert_eq!(format_issue_status(true), "✅ Completed");
        assert_eq!(format_issue_status(false), "🔄 Active");
    }
}
