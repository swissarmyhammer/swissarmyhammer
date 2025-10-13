//! Core types for Git operations
//!
//! This module provides type-safe wrappers and data structures
//! for Git operations to prevent common mistakes and improve API clarity.

use crate::error::{GitError, GitResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Type-safe wrapper for Git branch names
///
/// This newtype prevents confusion between branch names and other strings,
/// and provides validation for branch name syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    /// Create a new branch name with validation
    ///
    /// # Arguments
    /// * `name` - The branch name string
    ///
    /// # Returns
    /// * `Ok(BranchName)` if the name is valid
    /// * `Err(GitError)` if the name is invalid
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_git::BranchName;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let branch = BranchName::new("feature/user-auth")?;
    /// let main_branch = BranchName::new("main")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<S: Into<String>>(name: S) -> GitResult<Self> {
        let name = name.into();
        Self::validate_branch_name(&name)?;
        Ok(Self(name))
    }

    /// Create a new branch name without validation (use carefully)
    ///
    /// This should only be used when you're certain the name is valid,
    /// such as when reading existing branch names from Git.
    pub fn new_unchecked<S: Into<String>>(name: S) -> Self {
        Self(name.into())
    }

    /// Get the branch name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the branch name as a String
    pub fn into_string(self) -> String {
        self.0
    }

    /// Sanitize an issue name or other string for use as a Git branch name
    ///
    /// Replaces or removes characters that are invalid in Git branch names
    /// to create a valid branch name from potentially unsafe input.
    ///
    /// # Arguments
    /// * `name` - The string to sanitize
    ///
    /// # Returns
    /// A string that is safe to use as a Git branch name
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_git::BranchName;
    ///
    /// let sanitized = BranchName::sanitize("~BASE_WEAPON_TEMPLATES_22b9a174");
    /// assert_eq!(sanitized, "BASE_WEAPON_TEMPLATES_22b9a174");
    ///
    /// let sanitized = BranchName::sanitize("feature with spaces");
    /// assert_eq!(sanitized, "feature_with_spaces");
    /// ```
    pub fn sanitize(name: &str) -> String {
        let mut result = name.to_string();

        // Remove leading ~ (common in rule violation issue names)
        result = result.trim_start_matches('~').to_string();

        // Remove leading dashes
        result = result.trim_start_matches('-').to_string();

        // Replace spaces and tabs with underscores
        result = result.replace(' ', "_");
        result = result.replace('\t', "_");

        // Remove newlines
        result = result.replace('\n', "");
        result = result.replace('\r', "");

        // Replace double dots with single dot
        while result.contains("..") {
            result = result.replace("..", ".");
        }

        // Replace other invalid characters with underscores
        let invalid_chars = ['~', '^', ':', '?', '*', '[', '\\'];
        for &ch in &invalid_chars {
            result = result.replace(ch, "_");
        }

        // Ensure result is not empty
        if result.is_empty() {
            result = "branch".to_string();
        }

        result
    }

    /// Validate a branch name according to Git rules
    fn validate_branch_name(name: &str) -> GitResult<()> {
        if name.is_empty() {
            return Err(GitError::invalid_branch_name(
                name.to_string(),
                "Branch name cannot be empty".to_string(),
            ));
        }

        if name.starts_with('-') {
            return Err(GitError::invalid_branch_name(
                name.to_string(),
                "Branch name cannot start with a dash".to_string(),
            ));
        }

        if name.contains("..") {
            return Err(GitError::invalid_branch_name(
                name.to_string(),
                "Branch name cannot contain double dots (..)".to_string(),
            ));
        }

        if name.contains(' ') {
            return Err(GitError::invalid_branch_name(
                name.to_string(),
                "Branch name cannot contain spaces".to_string(),
            ));
        }

        if name.contains('\t') || name.contains('\n') {
            return Err(GitError::invalid_branch_name(
                name.to_string(),
                "Branch name cannot contain whitespace characters".to_string(),
            ));
        }

        // Check for invalid characters
        let invalid_chars = ['~', '^', ':', '?', '*', '[', '\\'];
        for &ch in &invalid_chars {
            if name.contains(ch) {
                return Err(GitError::invalid_branch_name(
                    name.to_string(),
                    format!("Branch name cannot contain '{}'", ch),
                ));
            }
        }

        Ok(())
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BranchName {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Information about a Git commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Commit hash (SHA-1)
    pub hash: String,
    /// Commit message
    pub message: String,
    /// Author name
    pub author: String,
    /// Author email
    pub author_email: String,
    /// Commit timestamp
    pub timestamp: DateTime<Utc>,
    /// Short commit hash (first 8 characters)
    pub short_hash: String,
}

impl CommitInfo {
    /// Create a new CommitInfo
    pub fn new(
        hash: String,
        message: String,
        author: String,
        author_email: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let short_hash = if hash.len() >= 8 {
            hash[..8].to_string()
        } else {
            hash.clone()
        };

        Self {
            hash,
            message,
            author,
            author_email,
            timestamp,
            short_hash,
        }
    }
}

/// Detailed status summary for git repository state
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusSummary {
    /// Files that are staged and modified
    pub staged_modified: Vec<String>,
    /// Files that are unstaged and modified
    pub unstaged_modified: Vec<String>,
    /// Files that are untracked
    pub untracked: Vec<String>,
    /// Files that are staged for addition
    pub staged_new: Vec<String>,
    /// Files that are staged for deletion
    pub staged_deleted: Vec<String>,
    /// Files that are deleted but not staged
    pub unstaged_deleted: Vec<String>,
    /// Files that are renamed
    pub renamed: Vec<String>,
    /// Files that have conflicts
    pub conflicted: Vec<String>,
}

impl StatusSummary {
    /// Create a new empty status summary
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the working directory is clean
    pub fn is_clean(&self) -> bool {
        self.staged_modified.is_empty()
            && self.unstaged_modified.is_empty()
            && self.untracked.is_empty()
            && self.staged_new.is_empty()
            && self.staged_deleted.is_empty()
            && self.unstaged_deleted.is_empty()
            && self.renamed.is_empty()
            && self.conflicted.is_empty()
    }

    /// Get all modified files (staged and unstaged)
    pub fn all_modified_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        files.extend(self.staged_modified.clone());
        files.extend(self.unstaged_modified.clone());
        files.sort();
        files.dedup();
        files
    }

    /// Get all files with changes (excluding untracked)
    pub fn all_changed_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        files.extend(self.staged_modified.clone());
        files.extend(self.unstaged_modified.clone());
        files.extend(self.staged_new.clone());
        files.extend(self.staged_deleted.clone());
        files.extend(self.unstaged_deleted.clone());
        files.extend(self.renamed.clone());
        files.sort();
        files.dedup();
        files
    }

    /// Check if there are any staged changes
    pub fn has_staged_changes(&self) -> bool {
        !self.staged_modified.is_empty()
            || !self.staged_new.is_empty()
            || !self.staged_deleted.is_empty()
    }

    /// Check if there are any unstaged changes
    pub fn has_unstaged_changes(&self) -> bool {
        !self.unstaged_modified.is_empty() || !self.unstaged_deleted.is_empty()
    }

    /// Check if there are any conflicts
    pub fn has_conflicts(&self) -> bool {
        !self.conflicted.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_name_valid() {
        assert!(BranchName::new("main").is_ok());
        assert!(BranchName::new("feature/user-auth").is_ok());
        assert!(BranchName::new("bugfix-123").is_ok());
        assert!(BranchName::new("dev_branch").is_ok());
    }

    #[test]
    fn test_branch_name_invalid() {
        assert!(BranchName::new("").is_err());
        assert!(BranchName::new("-branch").is_err());
        assert!(BranchName::new("branch..name").is_err());
        assert!(BranchName::new("branch with spaces").is_err());
        assert!(BranchName::new("branch~name").is_err());
        assert!(BranchName::new("branch^name").is_err());
        assert!(BranchName::new("branch:name").is_err());
    }

    #[test]
    fn test_branch_name_sanitize() {
        // Remove leading tilde (rule violation issues)
        assert_eq!(
            BranchName::sanitize("~BASE_WEAPON_TEMPLATES_22b9a174"),
            "BASE_WEAPON_TEMPLATES_22b9a174"
        );

        // Replace spaces with underscores
        assert_eq!(
            BranchName::sanitize("feature with spaces"),
            "feature_with_spaces"
        );

        // Remove leading dashes
        assert_eq!(BranchName::sanitize("-branch-name"), "branch-name");

        // Replace double dots with single dot
        assert_eq!(BranchName::sanitize("branch..name"), "branch.name");

        // Replace invalid characters with underscores
        assert_eq!(BranchName::sanitize("branch~name"), "branch_name");
        assert_eq!(BranchName::sanitize("branch^name"), "branch_name");
        assert_eq!(BranchName::sanitize("branch:name"), "branch_name");
        assert_eq!(BranchName::sanitize("branch?name"), "branch_name");
        assert_eq!(BranchName::sanitize("branch*name"), "branch_name");
        assert_eq!(BranchName::sanitize("branch[name]"), "branch_name]"); // Only [ is invalid, ] stays

        // Remove newlines
        assert_eq!(BranchName::sanitize("branch\nname"), "branchname");

        // Empty input becomes "branch"
        assert_eq!(BranchName::sanitize(""), "branch");
        assert_eq!(BranchName::sanitize("~"), "branch");
        assert_eq!(BranchName::sanitize("---"), "branch");

        // Complex combination
        assert_eq!(
            BranchName::sanitize("~feature: add~new^feature*with?spaces"),
            "feature__add_new_feature_with_spaces"
        );
    }

    #[test]
    fn test_status_summary_clean() {
        let status = StatusSummary::new();
        assert!(status.is_clean());
        assert!(!status.has_staged_changes());
        assert!(!status.has_unstaged_changes());
        assert!(!status.has_conflicts());
    }

    #[test]
    fn test_status_summary_with_changes() {
        let mut status = StatusSummary::new();
        status.staged_modified.push("file1.txt".to_string());
        status.unstaged_modified.push("file2.txt".to_string());

        assert!(!status.is_clean());
        assert!(status.has_staged_changes());
        assert!(status.has_unstaged_changes());

        let all_modified = status.all_modified_files();
        assert_eq!(all_modified, vec!["file1.txt", "file2.txt"]);
    }
}
