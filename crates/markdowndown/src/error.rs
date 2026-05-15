//! Error types for markdowndown

use thiserror::Error;

/// HTTP status code constants
const HTTP_STATUS_TOO_MANY_REQUESTS: u16 = 429;
const HTTP_STATUS_FORBIDDEN: u16 = 403;
const HTTP_STATUS_UNAUTHORIZED: u16 = 401;
const HTTP_STATUS_NOT_FOUND: u16 = 404;

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

impl Error {
    /// Check if this error matches a specific HTTP status code
    fn is_http_status(&self, code: u16) -> bool {
        match self {
            Error::Http(e) => e
                .status()
                .map(|s| s.as_u16() == code)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Check if this error is a rate limit error (HTTP 429)
    pub fn is_rate_limit(&self) -> bool {
        self.is_http_status(HTTP_STATUS_TOO_MANY_REQUESTS)
    }

    /// Check if this error is a forbidden error (HTTP 403)
    pub fn is_forbidden(&self) -> bool {
        self.is_http_status(HTTP_STATUS_FORBIDDEN)
    }

    /// Check if this error is an unauthorized error (HTTP 401)
    pub fn is_unauthorized(&self) -> bool {
        self.is_http_status(HTTP_STATUS_UNAUTHORIZED)
    }

    /// Check if this error is a not found error (HTTP 404)
    pub fn is_not_found(&self) -> bool {
        self.is_http_status(HTTP_STATUS_NOT_FOUND)
    }
}
