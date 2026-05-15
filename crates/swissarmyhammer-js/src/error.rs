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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluation_error_display() {
        let err = JsError::evaluation("something failed");
        assert_eq!(
            err.to_string(),
            "JavaScript evaluation error: something failed"
        );
    }

    #[test]
    fn test_evaluation_error_from_string() {
        let err = JsError::evaluation(String::from("owned message"));
        assert_eq!(
            err.to_string(),
            "JavaScript evaluation error: owned message"
        );
    }

    #[test]
    fn test_type_conversion_error_display() {
        let err = JsError::type_conversion("bad type");
        assert_eq!(err.to_string(), "Type conversion error: bad type");
    }

    #[test]
    fn test_runtime_error_display() {
        let err = JsError::runtime("init failed");
        assert_eq!(err.to_string(), "Runtime error: init failed");
    }

    #[test]
    fn test_variable_not_found_display() {
        let err = JsError::VariableNotFound {
            name: "missing_var".to_string(),
        };
        assert_eq!(err.to_string(), "Variable not found: missing_var");
    }

    #[test]
    fn test_lock_error_display() {
        let err = JsError::Lock("mutex poisoned".to_string());
        assert_eq!(err.to_string(), "Lock error: mutex poisoned");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = JsError::Timeout;
        assert_eq!(
            err.to_string(),
            "Timeout: expression exceeded maximum execution time"
        );
    }

    #[test]
    fn test_error_debug_impl() {
        let err = JsError::evaluation("test");
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Evaluation"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_result_type_alias() {
        // Verify the Result type alias works correctly
        let ok_result: Result<i32> = Ok(42);
        assert!(matches!(ok_result, Ok(42)));

        let err_result: Result<i32> = Err(JsError::evaluation("fail"));
        assert!(err_result.is_err());
    }
}
