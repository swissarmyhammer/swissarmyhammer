//! Error types for issue management operations

use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Result type for issue operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during issue management operations
#[derive(Error, Debug)]
pub enum Error {
    /// Issue not found with the given name
    #[error("Issue not found: {0}")]
    IssueNotFound(String),

    /// Issue already exists with the given identifier
    #[error("Issue already exists: {0}")]
    IssueAlreadyExists(u64),

    /// IO error occurred during file operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Git operation failed
    #[error("Git error: {0}")]
    Git(#[from] swissarmyhammer_git::GitError),

    /// Common utility error
    #[error("Common utility error: {0}")]
    Common(#[from] swissarmyhammer_common::SwissArmyHammerError),

    /// Generic error for other cases
    #[error("Issue management error: {0}")]
    Other(String),
}

impl Error {
    /// Create a generic error with a message
    pub fn other<S: Into<String>>(message: S) -> Self {
        Error::Other(message.into())
    }
}

impl From<Error> for swissarmyhammer_common::SwissArmyHammerError {
    fn from(error: Error) -> Self {
        match error {
            Error::IssueNotFound(name) => swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Issue not found: {}", name),
            },
            Error::IssueAlreadyExists(id) => swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Issue already exists: {}", id),
            },
            Error::Io(e) => {
                swissarmyhammer_common::SwissArmyHammerError::DirectoryCreation(e.to_string())
            }
            Error::Git(e) => swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Git error: {}", e),
            },
            Error::Common(e) => e, // Already the right type
            Error::Other(msg) => {
                swissarmyhammer_common::SwissArmyHammerError::Other { message: msg }
            }
        }
    }
}

impl Severity for Error {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Filesystem failures that prevent issue operations
            Error::Io(_) => ErrorSeverity::Critical,

            // Error: Issue operations that fail
            Error::IssueNotFound(_) => ErrorSeverity::Error,
            Error::Other(_) => ErrorSeverity::Error,

            // Warning: Issue already exists can be handled
            Error::IssueAlreadyExists(_) => ErrorSeverity::Warning,

            // Delegate to wrapped error's severity
            Error::Git(err) => err.severity(),
            Error::Common(err) => err.severity(),
        }
    }
}

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn test_issue_error_critical_severity() {
        // IO errors are critical
        let io_error = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(io_error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_issue_error_error_severity() {
        // Issue not found
        let not_found = Error::IssueNotFound("issue-123".to_string());
        assert_eq!(not_found.severity(), ErrorSeverity::Error);

        // Other error
        let other = Error::Other("something went wrong".to_string());
        assert_eq!(other.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_issue_error_warning_severity() {
        // Issue already exists is a warning
        let already_exists = Error::IssueAlreadyExists(12345);
        assert_eq!(already_exists.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_issue_error_git_delegation() {
        // Test that Git errors delegate to wrapped error's severity
        let git_error = swissarmyhammer_git::GitError::branch_not_found("main".to_string());
        let expected_severity = git_error.severity();

        let issue_error = Error::Git(git_error);
        assert_eq!(issue_error.severity(), expected_severity);
    }

    #[test]
    fn test_issue_error_common_delegation() {
        // Test that Common errors delegate to wrapped error's severity
        let common_error = swissarmyhammer_common::SwissArmyHammerError::DirectoryCreation(
            "failed to create directory".to_string(),
        );
        let expected_severity = common_error.severity();

        let issue_error = Error::Common(common_error);
        assert_eq!(issue_error.severity(), expected_severity);
    }
}
