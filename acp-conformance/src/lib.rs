//! ACP Conformance Test Suite
//!
//! This crate provides conformance testing for Agent Client Protocol (ACP) implementations.
//! Tests can be run against any implementation that provides the `Agent` trait.
//!
//! # Test Organization
//!
//! Tests are organized by protocol section:
//! - `initialization`: Protocol initialization and capability negotiation
//! - `sessions`: Session setup (new, load, set_mode)
//! - `content`: Content blocks (text, image, audio, embedded resources, resource links)
//! - `file_system`: File system access (read_text_file, write_text_file)
//! - `terminals`: Terminal command execution (create, output, wait_for_exit, kill, release)
//! - `agent_plan`: Agent planning and execution strategies (plan creation, updates, dynamic evolution)
//! - `slash_commands`: Slash command advertisement and invocation
//!
//! # Running Tests
//!
//! ```bash
//! # Run all conformance tests
//! cargo test
//! ```

pub mod agent_plan;
pub mod content;
pub mod file_system;
pub mod initialization;
pub mod responses;
pub mod sessions;
pub mod slash_commands;
pub mod terminals;
pub mod validation;

/// Result type for conformance tests
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during conformance testing
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Agent error: {0}")]
    Agent(#[from] agent_client_protocol::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_protocol() {
        let error = Error::Protocol("invalid handshake".to_string());
        assert_eq!(error.to_string(), "Protocol error: invalid handshake");
    }

    #[test]
    fn test_error_display_validation() {
        let error = Error::Validation("missing required field".to_string());
        assert_eq!(
            error.to_string(),
            "Validation error: missing required field"
        );
    }

    #[test]
    fn test_error_display_timeout() {
        let error = Error::Timeout("operation took too long".to_string());
        assert_eq!(error.to_string(), "Timeout: operation took too long");
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: Error = io_error.into();
        assert!(matches!(error, Error::Io(_)));
        assert!(error.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_from_json() {
        let json_str = "{invalid json}";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let error: Error = json_error.into();
        assert!(matches!(error, Error::Json(_)));
    }

    #[test]
    fn test_error_debug_format() {
        let error = Error::Protocol("test error".to_string());
        let debug_output = format!("{:?}", error);
        assert!(debug_output.contains("Protocol"));
        assert!(debug_output.contains("test error"));
    }

    #[test]
    fn test_result_type_ok() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }
        let result = returns_ok();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_type_err() {
        fn returns_err() -> Result<i32> {
            Err(Error::Validation("test error".to_string()))
        }
        let result = returns_err();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Validation(_)));
    }

    #[test]
    fn test_error_propagation() {
        fn inner() -> Result<()> {
            Err(Error::Protocol("inner error".to_string()))
        }

        fn outer() -> Result<()> {
            inner()?;
            Ok(())
        }

        let result = outer();
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Protocol(msg) => assert_eq!(msg, "inner error"),
            _ => panic!("Expected Protocol error"),
        }
    }
}
