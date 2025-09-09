//! Shared utilities for MCP operations
//!
//! This module provides common functionality used across MCP tool handlers
//! to reduce code duplication and ensure consistent behavior.

use rmcp::ErrorData as McpError;
use std::collections::HashMap;
use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_todo::TodoError;

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
                if message.contains("not found") {
                    McpError::invalid_params(message, None)
                } else if message.contains("already exists") {
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

    /// Convert TodoError to appropriate MCP error response
    pub fn handle_todo_error(error: TodoError, operation: &str) -> McpError {
        tracing::error!("MCP todo operation '{}' failed: {}", operation, error);

        match error {
            // User input validation errors
            TodoError::InvalidTodoListName(name) => {
                McpError::invalid_params(format!("Invalid todo list name: {name}"), None)
            }
            TodoError::InvalidTodoId(id) => {
                McpError::invalid_params(format!("Invalid todo item ID: {id}"), None)
            }
            TodoError::TodoListNotFound(name) => {
                McpError::invalid_params(format!("Todo list '{name}' not found"), None)
            }
            TodoError::TodoItemNotFound(id, list) => McpError::invalid_params(
                format!("Todo item '{id}' not found in list '{list}'"),
                None,
            ),
            TodoError::EmptyTask => {
                McpError::invalid_params("Task description cannot be empty".to_string(), None)
            }
            // System errors
            TodoError::Io(err) => McpError::internal_error(format!("IO error: {err}"), None),
            TodoError::Yaml(err) => McpError::internal_error(format!("YAML error: {err}"), None),
            TodoError::Common(common_err) => {
                // Since we're now using the common error type directly, just delegate
                Self::handle_error(common_err, operation)
            }
            TodoError::Other(msg) => {
                McpError::internal_error(format!("Todo operation failed: {msg}"), None)
            }
        }
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

    /// Format a memo preview with consistent formatting
    ///
    /// This provides standardized formatting for memo displays across all tools,
    /// ensuring consistent presentation in list, search, and other operations.
    pub fn format_memo_preview(
        memo: &swissarmyhammer_memoranda::Memo,
        preview_length: usize,
    ) -> String {
        format!(
            "• {}\n  Created: {}\n  Updated: {}\n  Preview: {}",
            memo.title,
            Self::format_timestamp(memo.created_at),
            Self::format_timestamp(memo.updated_at),
            Self::format_preview(memo.content.as_str(), preview_length)
        )
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
    fn test_formatter_memo_preview() {
        use swissarmyhammer_memoranda::{Memo, MemoContent, MemoTitle};

        let title = MemoTitle::new("Test Memo".to_string()).unwrap();
        let content = MemoContent::new("This is a long piece of content that should be truncated in the preview to show only the first part".to_string());
        let memo = Memo::new(title, content);

        let preview = McpFormatter::format_memo_preview(&memo, 50);
        assert!(preview.contains("Test Memo"));
        assert!(preview.contains("Created:"));
        assert!(preview.contains("Updated:"));
        assert!(preview.contains("Preview:"));
        assert!(preview.contains("This is a long piece of content"));
    }
}
