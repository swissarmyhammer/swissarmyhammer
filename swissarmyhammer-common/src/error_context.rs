//! Error handling utilities to reduce code duplication
//!
//! This module provides common error handling patterns used throughout
//! the SwissArmyHammer codebase.

use crate::SwissArmyHammerError;
use std::path::Path;

/// Helper for creating IO errors with formatted context
pub fn io_error_with_context<P: AsRef<Path>>(
    error: std::io::Error,
    path: P,
    action: &str,
) -> SwissArmyHammerError {
    SwissArmyHammerError::Io(std::io::Error::new(
        error.kind(),
        format!("{action} '{}': {error}", path.as_ref().display()),
    ))
}

/// Helper for creating IO errors with custom formatted message
pub fn io_error_with_message(error: std::io::Error, message: String) -> SwissArmyHammerError {
    SwissArmyHammerError::Io(std::io::Error::new(error.kind(), message))
}

/// Helper for converting any error to SwissArmyHammerError::Other
pub fn other_error<E: ToString>(error: E) -> SwissArmyHammerError {
    SwissArmyHammerError::Other {
        message: error.to_string(),
    }
}

/// Convenient type alias for map_err with io_error_with_context
pub type IoResult<T> = std::result::Result<T, std::io::Error>;

/// Extension trait for Result to add context helpers
pub trait IoResultExt<T> {
    /// Add context to an IO error with path and action description
    fn with_io_context<P: AsRef<Path>>(self, path: P, action: &str) -> crate::Result<T>;

    /// Add context with a custom message
    fn with_io_message(self, message: String) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError::Other
    fn to_other_error(self) -> crate::Result<T>;
}

impl<T, E: std::error::Error> IoResultExt<T> for std::result::Result<T, E> {
    fn with_io_context<P: AsRef<Path>>(self, path: P, action: &str) -> crate::Result<T> {
        self.map_err(|e| {
            SwissArmyHammerError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{action} '{}': {e}", path.as_ref().display()),
            ))
        })
    }

    fn with_io_message(self, message: String) -> crate::Result<T> {
        self.map_err(|e| {
            SwissArmyHammerError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{message}: {e}"),
            ))
        })
    }

    fn to_other_error(self) -> crate::Result<T> {
        self.map_err(other_error)
    }
}

// Note: std::io::Error already implements std::error::Error,
// so it's covered by the generic implementation above

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_io_error_with_context() {
        let path = PathBuf::from("/test/path.txt");
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let result = io_error_with_context(error, &path, "Failed to read file");

        match result {
            SwissArmyHammerError::Io(io_err) => {
                let message = io_err.to_string();
                assert!(message.contains("Failed to read file"));
                assert!(message.contains("/test/path.txt"));
                assert!(message.contains("file not found"));
            }
            _ => panic!("Expected IO error"),
        }
    }

    #[test]
    fn test_other_error() {
        let error_msg = "test error";
        let result = other_error(error_msg);

        match result {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "test error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_io_result_ext() {
        use std::fs;
        let path = PathBuf::from("/nonexistent/path.txt");
        let result: std::result::Result<String, std::io::Error> = fs::read_to_string(&path);
        let converted = result.with_io_context(&path, "Failed to read file");

        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Io(io_err) => {
                let message = io_err.to_string();
                assert!(message.contains("Failed to read file"));
                assert!(message.contains("/nonexistent/path.txt"));
            }
            _ => panic!("Expected IO error"),
        }
    }
}
