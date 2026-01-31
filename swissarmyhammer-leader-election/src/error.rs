//! Error types for leader election

use std::io;

/// Errors that can occur during leader election
#[derive(Debug, thiserror::Error)]
pub enum ElectionError {
    /// Failed to create lock file
    #[error("Failed to create lock file: {0}")]
    LockFileCreation(#[source] io::Error),

    /// Lock is held by another process
    #[error("Lock is held by another process")]
    LockHeld,

    /// Failed to acquire lock
    #[error("Failed to acquire lock: {0}")]
    LockAcquisition(#[source] io::Error),

    /// Socket path error
    #[error("Socket error: {0}")]
    SocketError(#[source] io::Error),
}

/// Result type for election operations
pub type Result<T> = std::result::Result<T, ElectionError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_election_error_display() {
        let err = ElectionError::LockHeld;
        assert_eq!(format!("{}", err), "Lock is held by another process");

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ElectionError::LockFileCreation(io_err);
        assert!(format!("{}", err).contains("Failed to create lock file"));
    }

    #[test]
    fn test_election_error_source() {
        let err = ElectionError::LockHeld;
        assert!(err.source().is_none());

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ElectionError::LockFileCreation(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_all_error_variants_display() {
        let lock_held = ElectionError::LockHeld;
        assert!(!format!("{}", lock_held).is_empty());

        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let creation = ElectionError::LockFileCreation(io_err);
        assert!(format!("{}", creation).contains("create lock file"));

        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let acquisition = ElectionError::LockAcquisition(io_err);
        assert!(format!("{}", acquisition).contains("acquire lock"));

        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let socket = ElectionError::SocketError(io_err);
        assert!(format!("{}", socket).contains("Socket error"));
    }
}
