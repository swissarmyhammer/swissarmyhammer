//! Error types for the JavaScript expression engine

use thiserror::Error;

/// Result type alias for JS operations
pub type Result<T> = std::result::Result<T, JsError>;

/// Errors that can occur during JavaScript expression evaluation
#[derive(Debug, Error)]
pub enum JsError {
    /// JavaScript evaluation error
    #[error("JavaScript evaluation error: {message}")]
    Evaluation { message: String },

    /// Variable not found in context
    #[error("Variable not found: {name}")]
    VariableNotFound { name: String },

    /// Failed to acquire lock on global state
    #[error("Lock error: {0}")]
    Lock(String),

    /// Type conversion error between Rust and JS
    #[error("Type conversion error: {message}")]
    TypeConversion { message: String },

    /// Runtime initialization or configuration error
    #[error("Runtime error: {message}")]
    Runtime { message: String },

    /// Expression exceeded maximum execution time
    #[error("Timeout: expression exceeded maximum execution time")]
    Timeout,
}

impl JsError {
    /// Create an evaluation error
    pub fn evaluation(msg: impl Into<String>) -> Self {
        Self::Evaluation {
            message: msg.into(),
        }
    }

    /// Create a type conversion error
    pub fn type_conversion(msg: impl Into<String>) -> Self {
        Self::TypeConversion {
            message: msg.into(),
        }
    }

    /// Create a runtime error
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime {
            message: msg.into(),
        }
    }
}
