//! Error types for code context operations

use swissarmyhammer_leader_election::ElectionError;

/// Errors that can occur during code context operations
#[derive(Debug, thiserror::Error)]
pub enum CodeContextError {
    /// An IO operation failed
    #[error("IO error")]
    Io(#[from] std::io::Error),

    /// A database operation failed
    #[error("database error")]
    Database(#[from] rusqlite::Error),

    /// Leader election failed
    #[error("election error")]
    Election(#[from] ElectionError),

    /// Invalid regex pattern
    #[error("invalid regex pattern: {0}")]
    Pattern(String),
}
