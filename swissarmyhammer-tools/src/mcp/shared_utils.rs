//! Shared utilities for MCP operations
//!
//! This module provides common functionality used across MCP tool handlers
//! to reduce code duplication and ensure consistent behavior.

use rmcp::ErrorData as McpError;
use std::collections::HashMap;
use swissarmyhammer_common::{Result, SwissArmyHammerError};

/// Type alias for common error type used across MCP tools
///
/// This provides a consistent error type for all MCP operations while maintaining
/// backwards compatibility with existing code. It serves as a bridge between the
/// domain-specific error types and the unified error handling system.
pub type CommonError = SwissArmyHammerError;

/// Standard response format for MCP operations
#[derive(Debug)]
pub struct McpResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Human-readable message describing the result
    pub message: String,
    /// Optional data payload for the response
    pub data: Option<HashMap<String, serde_json::Value>>,
}

impl McpResponse {
    /// Create a success response
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    /// Create a success response with data
    pub fn success_with_data(
        message: impl Into<String>,
        data: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

/// Common error handling patterns for MCP operations
pub struct McpErrorHandler;

impl McpErrorHandler {
    /// Convert SwissArmyHammerError to appropriate MCP error response
    ///
    /// This provides consistent error mapping across all MCP operations:
    /// - User input errors -> invalid_params
    /// - System errors -> internal_error
    /// - Security/validation errors -> invalid_params
    pub fn handle_error(error: SwissArmyHammerError, operation: &str) -> McpError {
        tracing::error!("MCP operation '{}' failed: {}", operation, error);

        match error {
            // File system errors
            SwissArmyHammerError::FileNotFound { path, suggestion } => McpError::invalid_params(
                format!("File not found: {path}. Suggestion: {suggestion}"),
                None,
            ),
            SwissArmyHammerError::NotAFile { path, suggestion } => McpError::invalid_params(
                format!("Path is not a file: {path}. Suggestion: {suggestion}"),
                None,
            ),
            SwissArmyHammerError::PermissionDenied {
                path,
                error,
                suggestion,
            } => McpError::invalid_params(
                format!("Permission denied: {path} - {error}. Suggestion: {suggestion}"),
                None,
            ),
            SwissArmyHammerError::InvalidFilePath { path, suggestion } => McpError::invalid_params(
                format!("Invalid file path: {path}. Suggestion: {suggestion}"),
                None,
            ),

            // System-level errors
            SwissArmyHammerError::Io(err) => {
                McpError::internal_error(format!("I/O error: {err}"), None)
            }
            SwissArmyHammerError::Serialization(err) => {
                McpError::internal_error(format!("Serialization error: {err}"), None)
            }
            SwissArmyHammerError::Json(err) => {
                McpError::internal_error(format!("JSON error: {err}"), None)
            }
            SwissArmyHammerError::Semantic { message } => {
                McpError::internal_error(format!("Search error: {message}"), None)
            }
            SwissArmyHammerError::Other { message } => {
                // Try to infer the error type from the message
                if message.contains("not found") || message.contains("already exists") {
                    McpError::invalid_params(message, None)
                } else {
                    McpError::internal_error(message, None)
                }
            }

            // Handle all other variants generically
            _ => McpError::internal_error(format!("Operation failed: {error}"), None),
        }
    }

    /// Handle results with consistent error mapping
    pub fn handle_result<T>(
        result: Result<T>,
        operation: &str,
    ) -> std::result::Result<T, McpError> {
        result.map_err(|e| Self::handle_error(e, operation))
    }
}

/// Validation utilities for MCP requests
pub struct McpValidation;

impl McpValidation {
    /// Validate string length
    pub fn validate_string_length(value: &str, field: &str, max_length: usize) -> Result<()> {
        if value.len() > max_length {
            return Err(SwissArmyHammerError::Other {
                message: format!(
                    "{} too long: {} characters (max: {})",
                    Self::capitalize_first_letter(field),
                    value.len(),
                    max_length
                ),
            });
        }
        Ok(())
    }

    /// Validate string is not empty
    pub fn validate_not_empty(value: &str, field: &str) -> Result<()> {
        if value.trim().is_empty() {
            return Err(SwissArmyHammerError::Other {
                message: format!("{} cannot be empty", Self::capitalize_first_letter(field)),
            });
        }
        Ok(())
    }

    /// Helper function to capitalize the first letter of a string
    fn capitalize_first_letter(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Validate identifier format (alphanumeric, hyphens, underscores only)
    pub fn validate_identifier(value: &str, field: &str) -> Result<()> {
        if value.is_empty() {
            return Err(SwissArmyHammerError::Other {
                message: format!("{} cannot be empty", Self::capitalize_first_letter(field)),
            });
        }

        for char in value.chars() {
            if !char.is_alphanumeric() && char != '-' && char != '_' {
                return Err(SwissArmyHammerError::Other { message: format!(
                    "{} contains invalid character: '{}'. Only alphanumeric characters, hyphens, and underscores are allowed",
                    Self::capitalize_first_letter(field),
                    char
                ) });
            }
        }

        Ok(())
    }

    /// Validate ULID format
    pub fn validate_ulid(value: &str, field: &str) -> Result<()> {
        if value.len() != 26 {
            return Err(SwissArmyHammerError::Other {
                message: format!("{field} must be 26 characters long (ULID format)"),
            });
        }

        for char in value.chars() {
            if !char.is_ascii_uppercase() && !char.is_ascii_digit() {
                return Err(SwissArmyHammerError::Other { message: format!(
                    "{field} contains invalid character: '{char}'. ULIDs must only contain uppercase letters and digits"
                ) });
            }
        }

        Ok(())
    }
}

/// Formatting utilities for consistent MCP responses
pub struct McpFormatter;

impl McpFormatter {
    /// Format a preview of long text content
    pub fn format_preview(content: &str, max_length: usize) -> String {
        if content.len() <= max_length {
            content.to_string()
        } else {
            format!("{}...", &content[..max_length])
        }
    }

    /// Format a timestamp in a consistent way
    pub fn format_timestamp(timestamp: chrono::DateTime<chrono::Utc>) -> String {
        timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    /// Format a file size in human-readable format
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Create a standardized summary for list operations
    pub fn format_list_summary(item_name: &str, count: usize, total: usize) -> String {
        if count == total {
            let plural_name = if count == 1 {
                item_name.to_string()
            } else {
                format!("{item_name}s")
            };
            format!("Found {count} {plural_name}")
        } else {
            let plural_name = if total == 1 {
                item_name.to_string()
            } else {
                format!("{item_name}s")
            };
            format!("Showing {count} of {total} {plural_name}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_response_creation() {
        let success = McpResponse::success("Operation completed");
        assert!(success.success);
        assert_eq!(success.message, "Operation completed");
        assert!(success.data.is_none());

        let error = McpResponse::error("Operation failed");
        assert!(!error.success);
        assert_eq!(error.message, "Operation failed");
    }

    #[test]
    fn test_validation_string_length() {
        assert!(McpValidation::validate_string_length("short", "field", 10).is_ok());
        assert!(McpValidation::validate_string_length("this is too long", "field", 10).is_err());
    }

    #[test]
    fn test_validation_not_empty() {
        assert!(McpValidation::validate_not_empty("content", "field").is_ok());
        assert!(McpValidation::validate_not_empty("", "field").is_err());
        assert!(McpValidation::validate_not_empty("   ", "field").is_err());
    }

    #[test]
    fn test_validation_identifier() {
        assert!(McpValidation::validate_identifier("valid_name", "field").is_ok());
        assert!(McpValidation::validate_identifier("valid-name", "field").is_ok());
        assert!(McpValidation::validate_identifier("valid123", "field").is_ok());
        assert!(McpValidation::validate_identifier("invalid name", "field").is_err());
        assert!(McpValidation::validate_identifier("invalid.name", "field").is_err());
    }

    #[test]
    fn test_formatter_preview() {
        let short = "short text";
        assert_eq!(McpFormatter::format_preview(short, 20), short);

        let long = "this is a very long text that needs to be truncated";
        let preview = McpFormatter::format_preview(long, 20);
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 23); // 20 + "..."
    }

    #[test]
    fn test_formatter_file_size() {
        assert_eq!(McpFormatter::format_file_size(512), "512 B");
        assert_eq!(McpFormatter::format_file_size(1536), "1.5 KB");
        assert_eq!(McpFormatter::format_file_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_formatter_list_summary() {
        assert_eq!(
            McpFormatter::format_list_summary("item", 1, 1),
            "Found 1 item"
        );
        assert_eq!(
            McpFormatter::format_list_summary("item", 5, 5),
            "Found 5 items"
        );
        assert_eq!(
            McpFormatter::format_list_summary("item", 3, 10),
            "Showing 3 of 10 items"
        );
    }

    #[test]
    fn test_formatter_list_summary_total_one_partial() {
        // When total == 1 but count < total (edge case: showing 0 of 1)
        assert_eq!(
            McpFormatter::format_list_summary("item", 0, 1),
            "Showing 0 of 1 item"
        );
    }

    #[test]
    fn test_formatter_timestamp() {
        use chrono::{TimeZone, Utc};
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 45).unwrap();
        let formatted = McpFormatter::format_timestamp(dt);
        assert_eq!(formatted, "2024-01-15 10:30:45 UTC");
    }

    // --- validate_ulid tests ---

    #[test]
    fn test_validate_ulid_valid() {
        // A valid ULID is 26 uppercase letters/digits
        let valid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        assert!(McpValidation::validate_ulid(valid, "id").is_ok());
    }

    #[test]
    fn test_validate_ulid_wrong_length() {
        assert!(McpValidation::validate_ulid("TOOSHORT", "id").is_err());
        assert!(McpValidation::validate_ulid("01ARZ3NDEKTSV4RRFFQ69G5FAVXX", "id").is_err());
    }

    #[test]
    fn test_validate_ulid_lowercase_rejected() {
        // Lowercase letters are not valid in ULID
        let lowercase = "01arz3ndektsv4rrffq69g5fav";
        assert!(McpValidation::validate_ulid(lowercase, "id").is_err());
    }

    #[test]
    fn test_validate_ulid_special_chars_rejected() {
        // 26 chars but contains a dash
        let with_dash = "01ARZ3NDEKTSV4RRFFQ69G5F-V";
        assert!(McpValidation::validate_ulid(with_dash, "id").is_err());
    }

    // --- McpErrorHandler::handle_error tests ---

    #[test]
    fn test_handle_error_file_not_found() {
        let err = SwissArmyHammerError::FileNotFound {
            path: "/tmp/missing.txt".to_string(),
            suggestion: "Check the path".to_string(),
        };
        let mcp_err = McpErrorHandler::handle_error(err, "test_op");
        let msg = format!("{:?}", mcp_err);
        assert!(msg.contains("File not found") || msg.contains("missing.txt"));
    }

    #[test]
    fn test_handle_error_permission_denied() {
        let err = SwissArmyHammerError::PermissionDenied {
            path: "/etc/passwd".to_string(),
            error: "read denied".to_string(),
            suggestion: "Run as root".to_string(),
        };
        let mcp_err = McpErrorHandler::handle_error(err, "test_op");
        let msg = format!("{:?}", mcp_err);
        assert!(msg.contains("Permission denied") || msg.contains("passwd"));
    }

    #[test]
    fn test_handle_error_io_error() {
        let err =
            SwissArmyHammerError::Io(std::io::Error::new(std::io::ErrorKind::Other, "disk full"));
        let mcp_err = McpErrorHandler::handle_error(err, "test_op");
        let msg = format!("{:?}", mcp_err);
        assert!(msg.contains("I/O error") || msg.contains("disk full"));
    }

    #[test]
    fn test_handle_error_other_not_found_variant() {
        let err = SwissArmyHammerError::Other {
            message: "task not found".to_string(),
        };
        let mcp_err = McpErrorHandler::handle_error(err, "test_op");
        let msg = format!("{:?}", mcp_err);
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_handle_error_other_generic_variant() {
        let err = SwissArmyHammerError::Other {
            message: "something went wrong".to_string(),
        };
        let mcp_err = McpErrorHandler::handle_error(err, "test_op");
        let msg = format!("{:?}", mcp_err);
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn test_handle_result_ok() {
        let ok: Result<i32> = Ok(42);
        let result = McpErrorHandler::handle_result(ok, "test_op");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_handle_result_err() {
        let err: Result<i32> = Err(SwissArmyHammerError::Other {
            message: "oops".to_string(),
        });
        let result = McpErrorHandler::handle_result(err, "test_op");
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_response_success_with_data() {
        let mut data = std::collections::HashMap::new();
        data.insert("key".to_string(), serde_json::json!("value"));
        let response = McpResponse::success_with_data("Done", data);
        assert!(response.success);
        assert!(response.data.is_some());
        assert_eq!(response.data.unwrap()["key"], serde_json::json!("value"));
    }
}
