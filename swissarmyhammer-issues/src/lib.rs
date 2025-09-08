//! Issue management and tracking system for SwissArmyHammer
//!
//! This crate provides a comprehensive issue tracking system that stores issues as markdown
//! files in a git repository. It's designed to be lightweight yet powerful, with features
//! like automatic numbering, git integration, and performance monitoring.
//!
//! ## Features
//!
//! - **Markdown-based Storage**: Issues are stored as markdown files with automatic numbering
//! - **Git Integration**: Automatic branch creation and management for issue workflows
//! - **Performance Monitoring**: Built-in metrics collection for performance analysis
//! - **Batch Operations**: Efficient batch creation, retrieval, and updates for large projects
//!
//! ## Basic Usage
//!
//! ```rust
//! use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new issue storage
//! let storage = FileSystemIssueStorage::new_default()?;
//!
//! // Create an issue
//! let issue = storage.create_issue(
//!     "fix_login_bug".to_string(),
//!     "# Login Bug\n\nUsers cannot log in with special characters.".to_string()
//! ).await?;
//!
//! println!("Created issue '{}'", issue.name);
//!
//! // List all issues
//! let issues = storage.list_issues().await?;
//! println!("Found {} issues", issues.len());
//!
//! // Mark as complete
//! let completed = storage.complete_issue(&issue.name).await?;
//! println!("Issue '{}' marked as complete", completed.name);
//! # Ok(())
//! # }
//! ```
//!
//! ## Issue Lifecycle
//!
//! ```rust
//! use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage, work_on_issue};
//! use swissarmyhammer_git::GitOperations;
//!
//! # async fn workflow_example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = FileSystemIssueStorage::new_default()?;
//! let git_ops = GitOperations::new()?;
//!
//! // 1. Create issue
//! let issue = storage.create_issue("new_feature".to_string(), "# New Feature\n\nDescription".to_string()).await?;
//!
//! // 2. Create work branch (name-based)  
//! let branch_result = work_on_issue(&issue.name, &storage, &git_ops).await?;
//!
//! // 3. Work on the issue...
//! // 4. Update issue with progress
//! let updated = storage.update_issue(&issue.name, "# New Feature\n\nDescription\n\n## Progress\n\nCompleted basic structure".to_string()).await?;
//!
//! // 5. Mark complete
//! let completed = storage.complete_issue(&issue.name).await?;
//!
//! // 6. Merge branch
//! // git_ops.merge_issue_branch_auto(&issue.name)?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod metrics;
pub mod storage;
pub mod types;
pub mod utils;

// Re-export main types for convenience
pub use error::{Error, Result};
pub use metrics::{MetricsSnapshot, Operation, PerformanceMetrics};
pub use storage::{FileSystemIssueStorage, IssueStorage};
pub use types::{Issue, IssueInfo, IssueName, IssueState};
pub use utils::{
    format_issue_status, get_content_from_args, get_current_issue_from_branch, get_project_status,
    work_on_issue, ContentSource, IssueBranchResult, IssueMergeResult, ProjectStatus,
};
