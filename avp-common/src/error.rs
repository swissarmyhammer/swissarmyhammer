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
