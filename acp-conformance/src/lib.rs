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
//! - `prompt_turn`: Prompt turn lifecycle (user message, agent processing, notifications, completion)
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
pub mod prompt_turn;
pub mod responses;
pub mod sessions;
pub mod slash_commands;
pub mod terminals;
pub mod tool_calls;
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

    #[test]
    fn test_all_error_variants_are_distinct() {
        // Ensure each error variant is distinguishable
        let protocol = Error::Protocol("msg".to_string());
        let validation = Error::Validation("msg".to_string());
        let timeout = Error::Timeout("msg".to_string());
        let io = Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "msg"));
        let json = Error::Json(serde_json::from_str::<()>("invalid").unwrap_err());

        // Verify distinct display outputs
        assert!(protocol.to_string().starts_with("Protocol"));
        assert!(validation.to_string().starts_with("Validation"));
        assert!(timeout.to_string().starts_with("Timeout"));
        assert!(io.to_string().starts_with("IO"));
        assert!(json.to_string().starts_with("JSON"));
    }

    #[test]
    fn test_error_with_empty_message() {
        let error = Error::Protocol("".to_string());
        assert_eq!(error.to_string(), "Protocol error: ");

        let error = Error::Validation("".to_string());
        assert_eq!(error.to_string(), "Validation error: ");
    }

    #[test]
    fn test_error_with_special_characters() {
        let error = Error::Protocol("Error with 'quotes' and \"double quotes\"".to_string());
        assert!(error.to_string().contains("'quotes'"));
        assert!(error.to_string().contains("\"double quotes\""));

        let error = Error::Validation("Unicode: 你好世界 🌍".to_string());
        assert!(error.to_string().contains("你好世界"));
        assert!(error.to_string().contains("🌍"));
    }

    #[test]
    fn test_io_error_conversion_preserves_kind() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: Error = io_error.into();

        match error {
            Error::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_error_chain_with_multiple_conversions() {
        fn parse_json() -> std::result::Result<(), serde_json::Error> {
            serde_json::from_str::<serde_json::Value>("invalid")?;
            Ok(())
        }

        fn do_work() -> Result<()> {
            parse_json()?;
            Ok(())
        }

        let result = do_work();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Json(_)));
    }

    #[test]
    fn test_result_type_with_complex_value() {
        fn returns_complex() -> Result<Vec<String>> {
            Ok(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        }

        let result = returns_complex();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_error_source_chain() {
        use std::error::Error as StdError;

        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "root cause");
        let error = Error::Io(io_error);

        // Verify the error has a source
        assert!(error.source().is_some());
    }
}
