//! Error types for HEB.

/// Errors that can occur in HEB operations.
#[derive(Debug, thiserror::Error)]
pub enum HebError {
    #[error("Database error: {0}")]
    Database(#[source] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[source] serde_json::Error),

    #[error("Election error: {0}")]
    Election(#[source] swissarmyhammer_leader_election::ElectionError),

    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),
}

pub type Result<T> = std::result::Result<T, HebError>;
