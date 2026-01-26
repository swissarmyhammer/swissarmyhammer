//! Error types for directory management operations.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using DirectoryError.
pub type Result<T> = std::result::Result<T, DirectoryError>;

/// Errors that can occur during directory operations.
#[derive(Error, Debug)]
pub enum DirectoryError {
    /// Not in a git repository (no .git found in parent directories).
    #[error("not in a git repository (no .git found)")]
    NotInGitRepository,

    /// Cannot determine home directory.
    #[error("cannot determine home directory")]
    NoHomeDirectory,

    /// Failed to create directory.
    #[error("failed to create directory '{path}': {source}")]
    DirectoryCreation {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to read file.
    #[error("failed to read file '{path}': {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to write file.
    #[error("failed to write file '{path}': {source}")]
    FileWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// File exceeds size limit.
    #[error("file '{path}' exceeds size limit: {size} bytes > {limit} bytes")]
    FileTooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },

    /// Path validation failed (potential path traversal).
    #[error("path validation failed for '{path}': potential path traversal")]
    PathValidation { path: PathBuf },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Other error with a message.
    #[error("{message}")]
    Other { message: String },
}

impl DirectoryError {
    /// Create a DirectoryCreation error.
    pub fn directory_creation(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::DirectoryCreation {
            path: path.into(),
            source,
        }
    }

    /// Create a FileRead error.
    pub fn file_read(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::FileRead {
            path: path.into(),
            source,
        }
    }

    /// Create a FileWrite error.
    pub fn file_write(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::FileWrite {
            path: path.into(),
            source,
        }
    }
}
