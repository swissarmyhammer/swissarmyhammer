//! MCP error conversion utilities
//!
//! This module provides common patterns for converting various error types
//! to SwissArmyHammerError, particularly for MCP-related operations.

use crate::SwissArmyHammerError;

/// Convert any error that implements Display/ToString to SwissArmyHammerError::Other
pub trait ToSwissArmyHammerError<T> {
    /// Convert the error to SwissArmyHammerError::Other
    fn to_swiss_error(self) -> crate::Result<T>;

    /// Convert the error to SwissArmyHammerError::Other with custom prefix
    fn to_swiss_error_with_context(self, context: &str) -> crate::Result<T>;
}

impl<T, E: std::fmt::Display> ToSwissArmyHammerError<T> for std::result::Result<T, E> {
    fn to_swiss_error(self) -> crate::Result<T> {
        self.map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        })
    }

    fn to_swiss_error_with_context(self, context: &str) -> crate::Result<T> {
        self.map_err(|e| SwissArmyHammerError::Other {
            message: format!("{context}: {e}"),
        })
    }
}

/// Common MCP error conversion functions
pub mod mcp {
    use super::*;

    /// Convert tantivy errors to SwissArmyHammerError
    pub fn tantivy_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("Search index error: {error}"),
        }
    }

    /// Convert serde errors to SwissArmyHammerError
    pub fn serde_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("Serialization error: {error}"),
        }
    }

    /// Convert JSON parsing errors to SwissArmyHammerError  
    pub fn json_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("JSON parsing error: {error}"),
        }
    }

    /// Convert template rendering errors to SwissArmyHammerError
    pub fn template_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("Template rendering error: {error}"),
        }
    }

    /// Convert workflow errors to SwissArmyHammerError
    pub fn workflow_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("Workflow error: {error}"),
        }
    }

    /// Convert validation errors to SwissArmyHammerError
    pub fn validation_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("Validation error: {error}"),
        }
    }

    /// Convert generic external library errors to SwissArmyHammerError
    pub fn external_error<E: std::fmt::Display>(library: &str, error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other {
            message: format!("{library} error: {error}"),
        }
    }
}

/// Extension trait for Result types to add MCP-specific error conversions
pub trait McpResultExt<T> {
    /// Convert to SwissArmyHammerError with tantivy context
    fn with_tantivy_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with serde context
    fn with_serde_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with JSON context
    fn with_json_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with template context
    fn with_template_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with workflow context
    fn with_workflow_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with validation context
    fn with_validation_context(self) -> crate::Result<T>;

    /// Convert to SwissArmyHammerError with custom external library context
    fn with_external_context(self, library: &str) -> crate::Result<T>;
}

impl<T, E: std::fmt::Display> McpResultExt<T> for std::result::Result<T, E> {
    fn with_tantivy_context(self) -> crate::Result<T> {
        self.map_err(mcp::tantivy_error)
    }

    fn with_serde_context(self) -> crate::Result<T> {
        self.map_err(mcp::serde_error)
    }

    fn with_json_context(self) -> crate::Result<T> {
        self.map_err(mcp::json_error)
    }

    fn with_template_context(self) -> crate::Result<T> {
        self.map_err(mcp::template_error)
    }

    fn with_workflow_context(self) -> crate::Result<T> {
        self.map_err(mcp::workflow_error)
    }

    fn with_validation_context(self) -> crate::Result<T> {
        self.map_err(mcp::validation_error)
    }

    fn with_external_context(self, library: &str) -> crate::Result<T> {
        self.map_err(|e| mcp::external_error(library, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_swiss_error() {
        let result: Result<i32, String> = Err("test error".to_string());
        let converted = result.to_swiss_error();

        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_to_swiss_error_with_context() {
        let result: Result<i32, String> = Err("original error".to_string());
        let converted = result.to_swiss_error_with_context("Failed operation");

        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Failed operation: original error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_tantivy_error() {
        let err = mcp::tantivy_error("idx corrupt");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Search index error: idx corrupt");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_serde_error() {
        let err = mcp::serde_error("bad bytes");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Serialization error: bad bytes");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_json_error() {
        let err = mcp::json_error("unexpected EOF");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "JSON parsing error: unexpected EOF");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_template_error() {
        let err = mcp::template_error("missing var");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Template rendering error: missing var");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_workflow_error() {
        let err = mcp::workflow_error("step 3 failed");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Workflow error: step 3 failed");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_validation_error() {
        let err = mcp::validation_error("field required");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Validation error: field required");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_external_error() {
        let err = mcp::external_error("Redis", "timeout");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert_eq!(message, "Redis error: timeout");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_tantivy() {
        let result: Result<i32, String> = Err("test error".to_string());
        let converted = result.with_tantivy_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Search index error: test error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_serde() {
        let result: Result<i32, String> = Err("bad data".to_string());
        let converted = result.with_serde_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Serialization error: bad data");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_json() {
        let result: Result<i32, String> = Err("unexpected token".to_string());
        let converted = result.with_json_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "JSON parsing error: unexpected token");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_template() {
        let result: Result<i32, String> = Err("missing variable".to_string());
        let converted = result.with_template_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Template rendering error: missing variable");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_workflow() {
        let result: Result<i32, String> = Err("step failed".to_string());
        let converted = result.with_workflow_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Workflow error: step failed");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_validation() {
        let result: Result<i32, String> = Err("invalid input".to_string());
        let converted = result.with_validation_context();
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "Validation error: invalid input");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext_external() {
        let result: Result<i32, String> = Err("connection refused".to_string());
        let converted = result.with_external_context("MyLibrary");
        assert!(converted.is_err());
        match converted.err().unwrap() {
            SwissArmyHammerError::Other { message: msg } => {
                assert_eq!(msg, "MyLibrary error: connection refused");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_to_swiss_error_ok_passes_through() {
        let result: Result<i32, String> = Ok(42);
        let converted = result.to_swiss_error();
        assert_eq!(converted.unwrap(), 42);
    }

    #[test]
    fn test_to_swiss_error_with_context_ok_passes_through() {
        let result: Result<i32, String> = Ok(99);
        let converted = result.to_swiss_error_with_context("ctx");
        assert_eq!(converted.unwrap(), 99);
    }

    #[test]
    fn test_mcp_result_ext_ok_passes_through() {
        let ok: Result<i32, String> = Ok(7);
        assert_eq!(ok.clone().with_tantivy_context().unwrap(), 7);
        assert_eq!(ok.clone().with_serde_context().unwrap(), 7);
        assert_eq!(ok.clone().with_json_context().unwrap(), 7);
        assert_eq!(ok.clone().with_template_context().unwrap(), 7);
        assert_eq!(ok.clone().with_workflow_context().unwrap(), 7);
        assert_eq!(ok.clone().with_validation_context().unwrap(), 7);
        assert_eq!(ok.with_external_context("Lib").unwrap(), 7);
    }

    #[test]
    fn test_mcp_external_error_includes_library_name() {
        // Verify that external_error properly interpolates the library parameter
        let err = mcp::external_error("CustomLib", "some failure");
        match err {
            SwissArmyHammerError::Other { message } => {
                assert!(message.starts_with("CustomLib error:"));
                assert!(message.ends_with("some failure"));
            }
            _ => panic!("Expected Other error"),
        }
    }
}
