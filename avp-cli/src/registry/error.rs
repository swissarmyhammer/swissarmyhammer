//! Typed error type for registry operations.

use std::fmt;

/// Error type for all registry and package management operations.
#[derive(Debug)]
pub enum RegistryError {
    /// Network or connection failure.
    Http(reqwest::Error),
    /// No credentials found -- user needs to `avp login`.
    AuthRequired,
    /// 401 -- token invalid or revoked.
    Unauthorized(String),
    /// 404 -- package or version not found.
    NotFound(String),
    /// 409 -- version already exists.
    Conflict(String),
    /// 403 -- not package owner.
    Forbidden(String),
    /// Other API error with status code and body.
    Api { status: u16, body: String },
    /// File system error.
    Io(std::io::Error),
    /// Local validation failure (e.g. invalid RuleSet structure).
    Validation(String),
    /// SHA-512 integrity mismatch.
    Integrity(String),
    /// JSON parsing error.
    Json(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "Network error: {}", e),
            Self::AuthRequired => write!(f, "Not logged in. Run 'avp login' first."),
            Self::Unauthorized(msg) => write!(f, "Authentication failed: {}", msg),
            Self::NotFound(msg) => write!(f, "{}", msg),
            Self::Conflict(msg) => write!(f, "Conflict: {}", msg),
            Self::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            Self::Api { status, body } => write!(f, "API error ({}): {}", status, body),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Validation(msg) => write!(f, "Validation error: {}", msg),
            Self::Integrity(msg) => write!(f, "Integrity error: {}", msg),
            Self::Json(msg) => write!(f, "JSON error: {}", msg),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for RegistryError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

impl From<std::io::Error> for RegistryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for RegistryError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}
