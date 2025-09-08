//! Error types for issue management operations

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
            Error::IssueNotFound(name) => {
                swissarmyhammer_common::SwissArmyHammerError::Other { 
                    message: format!("Issue not found: {}", name) 
                }
            }
            Error::IssueAlreadyExists(id) => {
                swissarmyhammer_common::SwissArmyHammerError::Other { 
                    message: format!("Issue already exists: {}", id) 
                }
            }
            Error::Io(e) => swissarmyhammer_common::SwissArmyHammerError::DirectoryCreation(e),
            Error::Git(e) => swissarmyhammer_common::SwissArmyHammerError::Other { 
                message: format!("Git error: {}", e) 
            },
            Error::Common(e) => e, // Already the right type
            Error::Other(msg) => swissarmyhammer_common::SwissArmyHammerError::Other { message: msg },
        }
    }
}