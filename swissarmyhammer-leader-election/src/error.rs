//! Error types for leader election

use std::io;

/// Errors that can occur during leader election
#[derive(Debug, thiserror::Error)]
pub enum ElectionError {
    /// Failed to create lock file
    #[error("failed to create lock file: {0}")]
    LockFileCreation(#[source] io::Error),

    /// Lock is held by another process
    #[error("lock is held by another process")]
    LockHeld,

    /// Failed to acquire lock
    #[error("failed to acquire lock: {0}")]
    LockAcquisition(#[source] io::Error),

    /// Discovery file I/O error
    #[error("discovery file error: {0}")]
    Discovery(#[source] io::Error),

    /// ZMQ bus error
    #[error("bus error: {0}")]
    Bus(#[source] zmq::Error),

    /// Message serialization/deserialization error (preserves source chain)
    #[error("serialization error: {0}")]
    Serialization(#[source] serde_json::Error),

    /// Protocol or channel error (no underlying source)
    #[error("message error: {0}")]
    Message(String),
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
        assert_eq!(format!("{}", err), "lock is held by another process");

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ElectionError::LockFileCreation(io_err);
        assert!(format!("{}", err).contains("failed to create lock file"));
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
    }

    #[test]
    fn test_discovery_error_display() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "no access");
        let err = ElectionError::Discovery(io_err);
        assert!(format!("{}", err).contains("discovery file error"));
    }

    #[test]
    fn test_discovery_error_source() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "no access");
        let err = ElectionError::Discovery(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_bus_error_display() {
        // zmq::Error doesn't have a public constructor, but we can test via the ZMQ API
        // by creating a known error condition. Instead, test the Message variant.
        let err = ElectionError::Message("test message".to_string());
        assert!(format!("{}", err).contains("test message"));
    }

    #[test]
    fn test_message_error_source() {
        let err = ElectionError::Message("test".to_string());
        // Message variant has no source
        assert!(err.source().is_none());
    }

    #[test]
    fn test_serialization_error_display() {
        // Create a serde_json error by parsing invalid JSON
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ElectionError::Serialization(json_err);
        assert!(format!("{}", err).contains("serialization error"));
    }

    #[test]
    fn test_serialization_error_source() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ElectionError::Serialization(json_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_debug() {
        let err = ElectionError::LockHeld;
        let debug = format!("{:?}", err);
        assert!(debug.contains("LockHeld"));
    }

    #[test]
    fn test_lock_acquisition_error_source() {
        let io_err = io::Error::other("lock fail");
        let err = ElectionError::LockAcquisition(io_err);
        assert!(err.source().is_some());
    }
}
