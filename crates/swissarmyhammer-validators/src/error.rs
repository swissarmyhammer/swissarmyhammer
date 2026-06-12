//! Error types for the AVP crate.

use thiserror::Error;

/// Main error type for AVP operations.
#[derive(Debug, Error)]
pub enum AvpError {
    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing or serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Context initialization or operation error.
    #[error("Context error: {0}")]
    Context(String),

    /// Validator parsing or loading error.
    #[error("Validator '{validator}' error: {message}")]
    Validator {
        /// The validator name or path.
        validator: String,
        /// The error message.
        message: String,
    },

    /// ACP agent error during prompt execution.
    #[error("Agent error: {0}")]
    Agent(String),

    /// The review's ACP client connection failed to stand up or died
    /// mid-review. Preserves the underlying connection error as `source()`
    /// so callers can walk or match the cause.
    #[error("review agent connection failed")]
    AgentConnection(#[source] agent_client_protocol::Error),

    /// File is a partial template, not a validator.
    ///
    /// This is not a true error - it indicates the file should be skipped
    /// during validator loading because it's a template partial.
    #[error("'{0}' is a partial, not a validator")]
    Partial(String),
}

impl AvpError {
    /// Check if this error indicates the file is a partial template.
    ///
    /// Partials are template includes, not validators, and should be
    /// silently skipped during loading.
    pub fn is_partial(&self) -> bool {
        matches!(self, AvpError::Partial(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avp_error_is_partial() {
        let partial = AvpError::Partial("test.md".to_string());
        assert!(partial.is_partial());

        let ctx_err = AvpError::Context("bad".to_string());
        assert!(!ctx_err.is_partial());
    }

    #[test]
    fn agent_connection_preserves_the_source_chain() {
        let err = AvpError::AgentConnection(agent_client_protocol::Error::internal_error());
        assert_eq!(err.to_string(), "review agent connection failed");
        let source = std::error::Error::source(&err)
            .expect("the underlying connection error must be walkable via source()");
        assert_eq!(
            source.to_string(),
            agent_client_protocol::Error::internal_error().to_string()
        );
    }

    #[test]
    fn test_avp_error_display() {
        let err = AvpError::Context("init failed".to_string());
        assert_eq!(err.to_string(), "Context error: init failed");

        let err = AvpError::Validator {
            validator: "no-secrets".to_string(),
            message: "parse failed".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Validator 'no-secrets' error: parse failed"
        );

        let err = AvpError::Agent("timeout".to_string());
        assert_eq!(err.to_string(), "Agent error: timeout");

        let err = AvpError::Partial("_partials/common.md".to_string());
        assert_eq!(
            err.to_string(),
            "'_partials/common.md' is a partial, not a validator"
        );
    }

    #[test]
    fn test_avp_error_from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let avp_err: AvpError = json_err.into();
        assert!(avp_err.to_string().contains("JSON error"));
    }
}
