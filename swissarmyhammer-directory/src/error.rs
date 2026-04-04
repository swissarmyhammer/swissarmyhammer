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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify Display output for NotInGitRepository.
    #[test]
    fn test_not_in_git_repository_display() {
        let err = DirectoryError::NotInGitRepository;
        assert_eq!(err.to_string(), "not in a git repository (no .git found)");
    }

    /// Verify Display output for NoHomeDirectory.
    #[test]
    fn test_no_home_directory_display() {
        let err = DirectoryError::NoHomeDirectory;
        assert_eq!(err.to_string(), "cannot determine home directory");
    }

    /// Verify Display output and constructor for DirectoryCreation.
    #[test]
    fn test_directory_creation_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = DirectoryError::directory_creation("/some/path", io_err);
        let msg = err.to_string();
        assert!(msg.contains("/some/path"));
        assert!(msg.contains("access denied"));
    }

    /// Verify Display output and constructor for FileRead.
    #[test]
    fn test_file_read_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = DirectoryError::file_read("/missing/file", io_err);
        let msg = err.to_string();
        assert!(msg.contains("/missing/file"));
        assert!(msg.contains("file not found"));
    }

    /// Verify Display output and constructor for FileWrite.
    #[test]
    fn test_file_write_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read only");
        let err = DirectoryError::file_write("/readonly/file", io_err);
        let msg = err.to_string();
        assert!(msg.contains("/readonly/file"));
        assert!(msg.contains("read only"));
    }

    /// Verify Display output for FileTooLarge.
    #[test]
    fn test_file_too_large_display() {
        let err = DirectoryError::FileTooLarge {
            path: PathBuf::from("/big/file"),
            size: 20_000_000,
            limit: 10_000_000,
        };
        let msg = err.to_string();
        assert!(msg.contains("/big/file"));
        assert!(msg.contains("20000000"));
        assert!(msg.contains("10000000"));
    }

    /// Verify Display output for PathValidation.
    #[test]
    fn test_path_validation_display() {
        let err = DirectoryError::PathValidation {
            path: PathBuf::from("/evil/../etc/passwd"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/evil/../etc/passwd"));
        assert!(msg.contains("path traversal"));
    }

    /// Verify Display output for Io variant (from conversion).
    #[test]
    fn test_io_error_from_conversion() {
        let io_err = std::io::Error::other("generic io error");
        let err: DirectoryError = io_err.into();
        let msg = err.to_string();
        assert!(msg.contains("generic io error"));
    }

    /// Verify Display output for Other variant.
    #[test]
    fn test_other_error_display() {
        let err = DirectoryError::Other {
            message: "custom error message".to_string(),
        };
        assert_eq!(err.to_string(), "custom error message");
    }

    /// Verify Debug output is available for all variants.
    #[test]
    fn test_debug_output() {
        let err = DirectoryError::NotInGitRepository;
        let debug = format!("{:?}", err);
        assert!(!debug.is_empty());

        let err2 = DirectoryError::Other {
            message: "test".to_string(),
        };
        let debug2 = format!("{:?}", err2);
        assert!(!debug2.is_empty());
    }
}
