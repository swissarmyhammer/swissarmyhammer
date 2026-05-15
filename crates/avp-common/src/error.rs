//! Error types for the AVP crate.

use thiserror::Error;

/// Main error type for AVP operations.
#[derive(Debug, Error)]
pub enum AvpError {
    /// IO error during stdin/stdout operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing or serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Unknown or unsupported hook type.
    #[error("Unknown hook type: {0}")]
    UnknownHookType(String),

    /// Missing required field in input.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Validation error during input processing.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Error during chain processing.
    #[error("Chain error: {0}")]
    Chain(#[from] ChainError),

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

    /// ACP agent error during validator execution.
    #[error("Agent error: {0}")]
    Agent(String),

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

/// Validation errors for hook inputs.
#[derive(Debug, Error, Clone)]
pub enum ValidationError {
    /// Input structure doesn't match expected schema.
    #[error("Invalid input structure: {0}")]
    InvalidStructure(String),

    /// Required field is missing from input.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Field value is invalid.
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidValue {
        /// The field that has an invalid value.
        field: String,
        /// The reason the value is invalid.
        reason: String,
    },
}

/// Errors that occur during chain processing.
#[derive(Debug, Error, Clone)]
pub enum ChainError {
    /// A chain link failed to process.
    #[error("Link '{link}' failed: {reason}")]
    LinkFailed {
        /// The name of the link that failed.
        link: String,
        /// The reason for failure.
        reason: String,
    },

    /// Aggregation of results failed.
    #[error("Aggregation failed: {0}")]
    AggregationFailed(String),

    /// Validation error during chain processing.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avp_error_is_partial() {
        let partial = AvpError::Partial("test.md".to_string());
        assert!(partial.is_partial());

        let io_err = AvpError::UnknownHookType("bad".to_string());
        assert!(!io_err.is_partial());
    }

    #[test]
    fn test_avp_error_display() {
        let err = AvpError::UnknownHookType("BadHook".to_string());
        assert_eq!(err.to_string(), "Unknown hook type: BadHook");

        let err = AvpError::MissingField("session_id".to_string());
        assert_eq!(err.to_string(), "Missing required field: session_id");

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

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::InvalidStructure("bad shape".to_string());
        assert_eq!(err.to_string(), "Invalid input structure: bad shape");

        let err = ValidationError::MissingField("tool_name".to_string());
        assert_eq!(err.to_string(), "Missing required field: tool_name");

        let err = ValidationError::InvalidValue {
            field: "severity".to_string(),
            reason: "must be info, warn, or error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid value for field 'severity': must be info, warn, or error"
        );
    }

    #[test]
    fn test_chain_error_display() {
        let err = ChainError::LinkFailed {
            link: "ValidatorExecutor".to_string(),
            reason: "agent crashed".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Link 'ValidatorExecutor' failed: agent crashed"
        );

        let err = ChainError::AggregationFailed("no outputs".to_string());
        assert_eq!(err.to_string(), "Aggregation failed: no outputs");
    }

    #[test]
    fn test_chain_error_from_validation_error() {
        let val_err = ValidationError::MissingField("test".to_string());
        let chain_err: ChainError = val_err.into();
        assert!(chain_err.to_string().contains("Validation error"));
    }

    #[test]
    fn test_avp_error_from_chain_error() {
        let chain_err = ChainError::LinkFailed {
            link: "test".to_string(),
            reason: "broke".to_string(),
        };
        let avp_err: AvpError = chain_err.into();
        assert!(avp_err.to_string().contains("Chain error"));
    }

    #[test]
    fn test_avp_error_from_validation_error() {
        let val_err = ValidationError::InvalidStructure("bad".to_string());
        let avp_err: AvpError = val_err.into();
        assert!(avp_err.to_string().contains("Validation error"));
    }
}
