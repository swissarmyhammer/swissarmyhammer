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
        self.map_err(|e| SwissArmyHammerError::Other(e.to_string()))
    }

    fn to_swiss_error_with_context(self, context: &str) -> crate::Result<T> {
        self.map_err(|e| SwissArmyHammerError::Other(format!("{context}: {e}")))
    }
}

/// Common MCP error conversion functions
pub mod mcp {
    use super::*;

    /// Convert tantivy errors to SwissArmyHammerError
    pub fn tantivy_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("Search index error: {error}"))
    }

    /// Convert serde errors to SwissArmyHammerError
    pub fn serde_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("Serialization error: {error}"))
    }

    /// Convert JSON parsing errors to SwissArmyHammerError  
    pub fn json_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("JSON parsing error: {error}"))
    }

    /// Convert template rendering errors to SwissArmyHammerError
    pub fn template_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("Template rendering error: {error}"))
    }

    /// Convert workflow errors to SwissArmyHammerError
    pub fn workflow_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("Workflow error: {error}"))
    }

    /// Convert validation errors to SwissArmyHammerError
    pub fn validation_error<E: std::fmt::Display>(error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("Validation error: {error}"))
    }

    /// Convert generic external library errors to SwissArmyHammerError
    pub fn external_error<E: std::fmt::Display>(library: &str, error: E) -> SwissArmyHammerError {
        SwissArmyHammerError::Other(format!("{library} error: {error}"))
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
            SwissArmyHammerError::Other(msg) => {
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
            SwissArmyHammerError::Other(msg) => {
                assert_eq!(msg, "Failed operation: original error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_error_functions() {
        let error = "test error";

        let tantivy = mcp::tantivy_error(error);
        match tantivy {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("Search index error"));
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected Other error"),
        }

        let serde = mcp::serde_error(error);
        match serde {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("Serialization error"));
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected Other error"),
        }

        let json = mcp::json_error(error);
        match json {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("JSON parsing error"));
                assert!(msg.contains("test error"));
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_mcp_result_ext() {
        let result: Result<i32, String> = Err("test error".to_string());

        let tantivy_result = result.clone().with_tantivy_context();
        assert!(tantivy_result.is_err());
        match tantivy_result.err().unwrap() {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("Search index error"));
            }
            _ => panic!("Expected Other error"),
        }

        let serde_result = result.clone().with_serde_context();
        assert!(serde_result.is_err());
        match serde_result.err().unwrap() {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("Serialization error"));
            }
            _ => panic!("Expected Other error"),
        }

        let external_result = result.with_external_context("MyLibrary");
        assert!(external_result.is_err());
        match external_result.err().unwrap() {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("MyLibrary error"));
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_all_mcp_error_types() {
        let error = "sample error";

        let template = mcp::template_error(error);
        match template {
            SwissArmyHammerError::Other(msg) => assert!(msg.contains("Template rendering error")),
            _ => panic!("Expected Other error"),
        }

        let workflow = mcp::workflow_error(error);
        match workflow {
            SwissArmyHammerError::Other(msg) => assert!(msg.contains("Workflow error")),
            _ => panic!("Expected Other error"),
        }

        let validation = mcp::validation_error(error);
        match validation {
            SwissArmyHammerError::Other(msg) => assert!(msg.contains("Validation error")),
            _ => panic!("Expected Other error"),
        }

        let external = mcp::external_error("TestLib", error);
        match external {
            SwissArmyHammerError::Other(msg) => {
                assert!(msg.contains("TestLib error"));
                assert!(msg.contains("sample error"));
            }
            _ => panic!("Expected Other error"),
        }
    }
}
