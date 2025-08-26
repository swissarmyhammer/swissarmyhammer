//! Issue management and tracking system
//!
//! This module provides a comprehensive issue tracking system that stores issues as markdown
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
//! use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
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
//! let completed = storage.mark_complete(&issue.name).await?;
//! println!("Issue '{}' marked as complete", completed.name);
//! # Ok(())
//! # }
//! ```
//!
//! ## Issue Lifecycle
//!
//! ```rust
//! use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
//! use swissarmyhammer::git::GitOperations;
//!
//! # async fn workflow_example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = FileSystemIssueStorage::new_default()?;
//! let git_ops = GitOperations::new()?;
//!
//! // 1. Create issue
//! let issue = storage.create_issue("new_feature".to_string(), "# New Feature\n\nDescription".to_string()).await?;
//!
//! // 2. Create work branch (name-based)  
//! let branch_name = git_ops.create_work_branch(&format!("issue/{}", issue.name))?;
//!
//! // 3. Work on the issue...
//! // 4. Update issue with progress
//! let updated = storage.update_issue(&issue.name, "# New Feature\n\nDescription\n\n## Progress\n\nCompleted basic structure".to_string()).await?;
//!
//! // 5. Mark complete
//! let completed = storage.mark_complete(&issue.name).await?;
//!
//! // 6. Merge branch
//! git_ops.merge_issue_branch_auto(&issue.name)?;
//! # Ok(())
//! # }
//! ```

use crate::config::Config;
use serde::{Deserialize, Serialize};

/// A wrapper type for issue names to prevent mixing up different string types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct IssueName(pub String);

impl IssueName {
    /// Create a new issue name with strict validation for MCP interface
    ///
    /// Uses configurable length limit and rejects filesystem-unsafe characters.
    /// Intended for user-provided input through the MCP interface.
    /// Empty names are allowed for nameless issues, but whitespace-only strings are rejected.
    pub fn new(name: String) -> Result<Self, String> {
        Self::new_with_config(name, Config::global())
    }

    /// Create a new issue name with custom config (for testing)
    pub fn new_with_config(name: String, config: &Config) -> Result<Self, String> {
        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err("Issue name cannot be empty".to_string());
        }

        if trimmed.len() > config.max_issue_name_length {
            return Err(format!(
                "Issue name cannot exceed {} characters",
                config.max_issue_name_length
            ));
        }

        // Check for invalid characters - reject problematic characters for MCP interface
        if trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0']) {
            return Err("Issue name contains invalid characters".to_string());
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Create a new issue name with relaxed validation for internal filesystem use
    ///
    /// Uses hard-coded length limit and allows filesystem-safe characters.
    /// Intended for filesystem operations and internal use.
    pub fn new_internal(name: String) -> Result<Self, String> {
        const FILESYSTEM_MAX_ISSUE_NAME_LENGTH: usize = 200;

        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err("Issue name cannot be empty".to_string());
        }

        if trimmed.len() > FILESYSTEM_MAX_ISSUE_NAME_LENGTH {
            return Err(format!(
                "Issue name cannot exceed {FILESYSTEM_MAX_ISSUE_NAME_LENGTH} characters"
            ));
        }

        // More permissive validation for filesystem operations
        if trimmed.contains(['\0', '/']) {
            return Err("Issue name contains invalid characters".to_string());
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Create a new issue name with relaxed validation for internal filesystem use
    ///
    /// Uses a fixed length limit and only rejects null bytes.
    /// Intended for parsing existing filenames from the filesystem.
    /// Empty names are allowed for nameless issues like 000123.md, but whitespace-only strings are rejected.
    pub fn from_filesystem(name: String) -> Result<Self, String> {
        const FILESYSTEM_MAX_ISSUE_NAME_LENGTH: usize = 200;

        let trimmed = name.trim();

        // Allow truly empty names for nameless issues, but reject whitespace-only strings
        if name.trim().is_empty() && !name.is_empty() {
            return Err("Issue name cannot be empty".to_string());
        }

        // For filesystem names, allow up to a fixed limit and only reject null bytes
        if trimmed.len() > FILESYSTEM_MAX_ISSUE_NAME_LENGTH {
            return Err(format!(
                "Issue name cannot exceed {FILESYSTEM_MAX_ISSUE_NAME_LENGTH} characters"
            ));
        }

        // Only reject null bytes for filesystem names
        if trimmed.contains('\0') {
            return Err("Issue name contains invalid characters".to_string());
        }

        Ok(IssueName(trimmed.to_string()))
    }

    /// Get the inner string value (alias for as_str)
    pub fn get(&self) -> &str {
        &self.0
    }

    /// Create from string with validation (alias for new)
    pub fn from_string(name: String) -> Result<Self, String> {
        Self::new(name)
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner string value
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for IssueName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for IssueName {
    fn from(name: String) -> Self {
        IssueName(name)
    }
}

impl From<&str> for IssueName {
    fn from(name: &str) -> Self {
        IssueName(name.to_string())
    }
}

impl From<IssueName> for String {
    fn from(issue_name: IssueName) -> Self {
        issue_name.0
    }
}

impl AsRef<str> for IssueName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Filesystem-based issue storage implementation
pub mod filesystem;
/// Performance metrics collection and analysis
pub mod metrics;
/// Shared utilities for issue management
pub mod utils;

// Re-export main types from the filesystem module
pub use filesystem::{
    create_safe_filename, extract_issue_name_from_filename, format_issue_number,
    get_issue_name_from_filename, is_issue_file, parse_issue_filename, parse_issue_number,
    sanitize_issue_name, validate_issue_name, FileSystemIssueStorage, Issue, IssueInfo, IssueState,
    IssueStorage,
};

// Export metrics types
pub use metrics::{MetricsSnapshot, Operation, PerformanceMetrics};

// Export utilities
pub use utils::{
    format_issue_status, get_content_from_args, get_current_issue_from_branch, get_project_status,
    work_on_issue, ContentSource, IssueBranchResult, IssueMergeResult, ProjectStatus,
};
