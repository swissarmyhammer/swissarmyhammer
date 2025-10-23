//! Git-specific error types and handling
//!
//! This module provides comprehensive error types for Git operations,
//! with detailed context and recovery suggestions.

use std::path::PathBuf;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Result type for Git operations
pub type GitResult<T> = Result<T, GitError>;

/// Comprehensive Git error types
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GitError {
    /// Repository not found or invalid
    #[error("Git repository not found at '{path}': {details}")]
    RepositoryNotFound { path: PathBuf, details: String },

    /// Repository operation failed
    #[error("Git repository operation '{operation}' failed: {details}")]
    RepositoryOperationFailed { operation: String, details: String },

    /// Branch operation failed
    #[error("Git branch operation '{operation}' on branch '{branch}' failed: {details}")]
    BranchOperationFailed {
        operation: String,
        branch: String,
        details: String,
    },

    /// Branch not found
    #[error("Git branch '{branch}' not found")]
    BranchNotFound { branch: String },

    /// Branch already exists
    #[error("Git branch '{branch}' already exists")]
    BranchAlreadyExists { branch: String },

    /// Commit operation failed
    #[error("Git commit operation '{operation}' failed: {details}")]
    CommitOperationFailed { operation: String, details: String },

    /// Merge operation failed
    #[error("Git merge operation failed: {details}")]
    MergeOperationFailed { details: String },

    /// Working directory is dirty
    #[error("Git working directory has uncommitted changes: {files:?}")]
    WorkingDirectoryDirty { files: Vec<String> },

    /// Invalid branch name
    #[error("Invalid branch name '{name}': {reason}")]
    InvalidBranchName { name: String, reason: String },

    /// Git2 library error
    #[error("Git2 operation '{operation}' failed: {source}")]
    Git2Error {
        operation: String,
        #[source]
        source: git2::Error,
    },

    /// IO error during git operations
    #[error("IO error during git operation '{operation}': {source}")]
    IoError {
        operation: String,
        #[source]
        source: std::io::Error,
    },

    /// Generic git error
    #[error("Git error: {message}")]
    Generic { message: String },
}

impl GitError {
    /// Create a repository not found error
    pub fn repository_not_found<P: Into<PathBuf>>(path: P, details: String) -> Self {
        Self::RepositoryNotFound {
            path: path.into(),
            details,
        }
    }

    /// Create a repository operation failed error
    pub fn repository_operation_failed(operation: String, details: String) -> Self {
        Self::RepositoryOperationFailed { operation, details }
    }

    /// Create a branch operation failed error
    pub fn branch_operation_failed(operation: String, branch: String, details: String) -> Self {
        Self::BranchOperationFailed {
            operation,
            branch,
            details,
        }
    }

    /// Create a branch not found error
    pub fn branch_not_found(branch: String) -> Self {
        Self::BranchNotFound { branch }
    }

    /// Create a branch already exists error
    pub fn branch_already_exists(branch: String) -> Self {
        Self::BranchAlreadyExists { branch }
    }

    /// Create a commit operation failed error
    pub fn commit_operation_failed(operation: String, details: String) -> Self {
        Self::CommitOperationFailed { operation, details }
    }

    /// Create a merge operation failed error
    pub fn merge_operation_failed(details: String) -> Self {
        Self::MergeOperationFailed { details }
    }

    /// Create a working directory dirty error
    pub fn working_directory_dirty(files: Vec<String>) -> Self {
        Self::WorkingDirectoryDirty { files }
    }

    /// Create an invalid branch name error
    pub fn invalid_branch_name(name: String, reason: String) -> Self {
        Self::InvalidBranchName { name, reason }
    }

    /// Create a git2 error with operation context
    pub fn from_git2(operation: String, error: git2::Error) -> Self {
        Self::Git2Error {
            operation,
            source: error,
        }
    }

    /// Create an IO error with operation context
    pub fn from_io(operation: String, error: std::io::Error) -> Self {
        Self::IoError {
            operation,
            source: error,
        }
    }

    /// Create a generic git error
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Create a generic git error from string (convenience method)
    pub fn from_string(message: String) -> Self {
        Self::Generic { message }
    }
}

/// Convert git2::Error to GitError with operation context
pub fn convert_git2_error(operation: &str, error: git2::Error) -> GitError {
    GitError::from_git2(operation.to_string(), error)
}

/// Convert std::io::Error to GitError with operation context
pub fn convert_io_error(operation: &str, error: std::io::Error) -> GitError {
    GitError::from_io(operation.to_string(), error)
}

impl Severity for GitError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Filesystem failures that prevent git operations
            GitError::IoError { .. } => ErrorSeverity::Critical,

            // Critical: Merge conflicts require immediate resolution
            GitError::MergeOperationFailed { .. } => ErrorSeverity::Critical,

            // Error: Operations that fail but don't corrupt state
            GitError::RepositoryNotFound { .. } => ErrorSeverity::Error,
            GitError::RepositoryOperationFailed { .. } => ErrorSeverity::Error,
            GitError::BranchOperationFailed { .. } => ErrorSeverity::Error,
            GitError::BranchNotFound { .. } => ErrorSeverity::Error,
            GitError::CommitOperationFailed { .. } => ErrorSeverity::Error,
            GitError::InvalidBranchName { .. } => ErrorSeverity::Error,
            GitError::Git2Error { .. } => ErrorSeverity::Error,
            GitError::Generic { .. } => ErrorSeverity::Error,

            // Warning: Informational state that doesn't prevent operations
            GitError::BranchAlreadyExists { .. } => ErrorSeverity::Warning,
            GitError::WorkingDirectoryDirty { .. } => ErrorSeverity::Warning,
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn test_git_error_critical_severity() {
        // IO errors are critical
        let io_error = GitError::from_io(
            "test operation".to_string(),
            std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        );
        assert_eq!(io_error.severity(), ErrorSeverity::Critical);

        // Merge operation failures are critical
        let merge_error = GitError::merge_operation_failed("conflict detected".to_string());
        assert_eq!(merge_error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_git_error_error_severity() {
        // Repository not found
        let repo_error =
            GitError::repository_not_found("/path/to/repo", "not a git repository".to_string());
        assert_eq!(repo_error.severity(), ErrorSeverity::Error);

        // Repository operation failed
        let repo_op_error = GitError::repository_operation_failed(
            "clone".to_string(),
            "failed to clone".to_string(),
        );
        assert_eq!(repo_op_error.severity(), ErrorSeverity::Error);

        // Branch operation failed
        let branch_op_error = GitError::branch_operation_failed(
            "checkout".to_string(),
            "main".to_string(),
            "branch not found".to_string(),
        );
        assert_eq!(branch_op_error.severity(), ErrorSeverity::Error);

        // Branch not found
        let branch_error = GitError::branch_not_found("feature".to_string());
        assert_eq!(branch_error.severity(), ErrorSeverity::Error);

        // Commit operation failed
        let commit_error = GitError::commit_operation_failed(
            "commit".to_string(),
            "nothing to commit".to_string(),
        );
        assert_eq!(commit_error.severity(), ErrorSeverity::Error);

        // Invalid branch name
        let invalid_branch = GitError::invalid_branch_name(
            "bad/branch".to_string(),
            "contains invalid characters".to_string(),
        );
        assert_eq!(invalid_branch.severity(), ErrorSeverity::Error);

        // Generic error
        let generic_error = GitError::generic("something went wrong");
        assert_eq!(generic_error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_git_error_warning_severity() {
        // Branch already exists
        let branch_exists = GitError::branch_already_exists("feature".to_string());
        assert_eq!(branch_exists.severity(), ErrorSeverity::Warning);

        // Working directory dirty
        let dirty_wd = GitError::working_directory_dirty(vec![
            "file1.txt".to_string(),
            "file2.txt".to_string(),
        ]);
        assert_eq!(dirty_wd.severity(), ErrorSeverity::Warning);
    }
}
