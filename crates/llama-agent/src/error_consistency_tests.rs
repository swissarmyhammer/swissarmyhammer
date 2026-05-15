//! Tests to ensure error handling consistency across modules
//!
//! This module verifies that all error types implement the LlamaError trait
//! consistently and provide proper categorization, error codes, and user-friendly messages.

#[cfg(test)]
mod tests {
    use llama_common::error::{ErrorCategory, LlamaError};
    use std::time::Duration;

    use crate::types::{AgentError, MCPError, QueueError, SessionError};

    #[test]
    fn test_agent_error_categorization() {
        let timeout_error = AgentError::Timeout {
            timeout: Duration::from_secs(30),
        };
        // AgentError should categorize timeout as System error
        assert_eq!(timeout_error.category(), ErrorCategory::System);
        assert!(timeout_error.is_retriable());
        assert!(!timeout_error.is_user_error());
    }

    #[test]
    fn test_queue_error_categorization() {
        let queue_full = QueueError::Full;
        // Queue full should be categorized as System error (resource constraint)
        assert_eq!(queue_full.category(), ErrorCategory::System);
        assert!(queue_full.is_retriable());

        let worker_error = QueueError::WorkerError("model failed".to_string());
        // Worker errors could be various categories, but default to System
        assert_eq!(worker_error.category(), ErrorCategory::System);
    }

    #[test]
    fn test_mcp_error_categorization() {
        let connection_error = MCPError::Connection("network unavailable".to_string());
        // Connection errors should be External category
        assert_eq!(connection_error.category(), ErrorCategory::External);
        assert!(connection_error.is_retriable());

        let protocol_error = MCPError::Protocol("invalid json".to_string());
        // Protocol errors could be user errors (malformed data) or external
        assert_eq!(protocol_error.category(), ErrorCategory::External);
    }

    #[test]
    fn test_session_error_categorization() {
        let not_found = SessionError::NotFound("session_123".to_string());
        // Session not found is typically a user error (invalid ID provided)
        assert_eq!(not_found.category(), ErrorCategory::User);
        assert!(!not_found.is_retriable());

        let limit_exceeded = SessionError::LimitExceeded;
        // Session limit is a system constraint
        assert_eq!(limit_exceeded.category(), ErrorCategory::System);
        assert!(!limit_exceeded.is_retriable()); // Not retriable without configuration change
    }

    #[test]
    fn test_error_codes_are_unique() {
        let timeout_error = AgentError::Timeout {
            timeout: Duration::from_secs(30),
        };
        let queue_full = AgentError::QueueFull { capacity: 100 };

        // Error codes should be unique and meaningful
        assert_ne!(timeout_error.error_code(), queue_full.error_code());
        assert!(timeout_error.error_code().contains("TIMEOUT"));
        assert!(queue_full.error_code().contains("QUEUE"));
    }

    #[test]
    fn test_user_friendly_messages_contain_advice() {
        let timeout_error = AgentError::Timeout {
            timeout: Duration::from_secs(30),
        };
        let message = timeout_error.user_friendly_message();

        // Should contain actionable advice
        assert!(
            message.contains("💡")
                || message.contains("Tip")
                || message.to_lowercase().contains("try")
        );
        assert!(!message.is_empty());
    }

    #[test]
    fn test_recovery_suggestions_provided() {
        let connection_error = MCPError::Connection("failed to connect".to_string());
        let suggestions = connection_error.recovery_suggestions();

        // Should provide at least one recovery suggestion
        assert!(!suggestions.is_empty());
        // External errors should suggest connectivity checks
        assert!(suggestions
            .iter()
            .any(|s| s.to_lowercase().contains("network")
                || s.to_lowercase().contains("connectivity")
                || s.to_lowercase().contains("retry")));
    }

    #[test]
    fn test_error_chain_preservation() {
        use std::io;

        // Create a chain: IO Error -> QueueError -> AgentError
        let io_error = io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused");
        let queue_error = QueueError::WorkerError(format!("IO failed: {}", io_error));
        let agent_error = AgentError::Queue(queue_error);

        // The error chain should be preserved for debugging
        let error_chain = format!("{:?}", agent_error);
        assert!(error_chain.contains("WorkerError"));
        assert!(error_chain.contains("IO failed"));
    }
}
