//! Error types for HEB.

/// Errors that can occur in HEB operations.
#[derive(Debug, thiserror::Error)]
pub enum HebError {
    #[error("database error: {0}")]
    Database(#[source] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[source] serde_json::Error),

    #[error("election error: {0}")]
    Election(#[source] swissarmyhammer_leader_election::ElectionError),

    #[error("io error: {0}")]
    Io(#[source] std::io::Error),
}

pub type Result<T> = std::result::Result<T, HebError>;
