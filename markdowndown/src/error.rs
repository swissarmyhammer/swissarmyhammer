//! Error types for markdowndown

use thiserror::Error;

/// Result type for markdowndown operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur during HTML to Markdown conversion
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// URL parsing error
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// Generic error
    #[error("{0}")]
    Other(String),
}
