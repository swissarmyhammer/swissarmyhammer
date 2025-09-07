//! Error types for SwissArmyHammer Common
//!
//! This module provides structured error handling for common operations
//! throughout the SwissArmyHammer ecosystem.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for SwissArmyHammer Common operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Common error types for SwissArmyHammer operations
#[derive(Error, Debug)]
pub enum SwissArmyHammerError {
    /// Not currently in a Git repository
    #[error("Not in a Git repository. SwissArmyHammer requires being run from within a Git repository.")]
    NotInGitRepository,

    /// Failed to create directory
    #[error("Failed to create directory: {0}")]
    DirectoryCreation(#[from] std::io::Error),

    /// Invalid path encountered
    #[error("Invalid path: {path}")]
    InvalidPath {
        /// The invalid path that caused the error
        path: PathBuf
    },

    /// Permission denied error
    #[error("Permission denied accessing: {path}")]
    PermissionDenied {
        /// The path that could not be accessed due to permission restrictions
        path: PathBuf
    },

    /// General I/O error with context
    #[error("I/O error: {message}")]
    Io {
        /// Descriptive message about the I/O error
        message: String
    },

    /// Other error with custom message
    #[error("{message}")]
    Other {
        /// Custom error message
        message: String
    },
}

impl SwissArmyHammerError {
    /// Create a new directory creation error
    pub fn directory_creation(error: std::io::Error) -> Self {
        Self::DirectoryCreation(error)
    }

    /// Create a new invalid path error
    pub fn invalid_path(path: PathBuf) -> Self {
        Self::InvalidPath { path }
    }

    /// Create a new permission denied error
    pub fn permission_denied(path: PathBuf) -> Self {
        Self::PermissionDenied { path }
    }

    /// Create a new I/O error with context
    pub fn io(message: String) -> Self {
        Self::Io { message }
    }

    /// Create a new other error
    pub fn other(message: String) -> Self {
        Self::Other { message }
    }
}