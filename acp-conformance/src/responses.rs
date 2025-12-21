//! Type-safe response wrappers for ACP extension requests.
//!
//! This module provides strongly-typed wrappers around JSON responses from
//! file system and terminal operations, eliminating manual JSON parsing boilerplate
//! and reducing errors.

use crate::{validation, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Response from a file system read operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileReadResponse {
    /// The content of the file that was read.
    pub content: String,
}

impl FileReadResponse {
    /// Creates a new FileReadResponse.
    pub fn new(content: String) -> Self {
        Self { content }
    }

    /// Parses a FileReadResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(FileReadResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let content = validation::require_string_field(value, "content")?;
        Ok(Self {
            content: content.to_string(),
        })
    }
}

/// Response from a file system write operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileWriteResponse {
    /// Whether the write operation was successful.
    pub success: bool,
}

impl FileWriteResponse {
    /// Creates a new FileWriteResponse.
    pub fn new(success: bool) -> Self {
        Self { success }
    }

    /// Parses a FileWriteResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(FileWriteResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let success = validation::require_bool_field(value, "success")?;
        Ok(Self { success })
    }
}

/// Response from a terminal creation operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalCreateResponse {
    /// The unique identifier for the created terminal.
    pub terminal_id: String,
}

impl TerminalCreateResponse {
    /// Creates a new TerminalCreateResponse.
    pub fn new(terminal_id: String) -> Self {
        Self { terminal_id }
    }

    /// Parses a TerminalCreateResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(TerminalCreateResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let terminal_id = validation::require_string_field(value, "terminal_id")?;
        Ok(Self {
            terminal_id: terminal_id.to_string(),
        })
    }
}

/// Response from a terminal output retrieval operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalOutputResponse {
    /// The output from the terminal.
    pub output: String,
    /// Whether the output was truncated.
    #[serde(default)]
    pub truncated: bool,
}

impl TerminalOutputResponse {
    /// Creates a new TerminalOutputResponse.
    pub fn new(output: String, truncated: bool) -> Self {
        Self { output, truncated }
    }

    /// Parses a TerminalOutputResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(TerminalOutputResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let output = validation::require_string_field(value, "output")?;
        let truncated = value
            .get("truncated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self {
            output: output.to_string(),
            truncated,
        })
    }
}

/// Response from a terminal wait_for_exit operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalExitResponse {
    /// The exit code of the terminal process.
    pub exit_code: i32,
    /// Whether the process was signaled.
    #[serde(default)]
    pub signaled: bool,
}

impl TerminalExitResponse {
    /// Creates a new TerminalExitResponse.
    pub fn new(exit_code: i32, signaled: bool) -> Self {
        Self {
            exit_code,
            signaled,
        }
    }

    /// Parses a TerminalExitResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(TerminalExitResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        // exit_code might be i32 or u64, handle both
        let exit_code = if let Some(code) = value.get("exit_code").and_then(|v| v.as_i64()) {
            code as i32
        } else {
            return Err(Error::Validation(
                "Field 'exit_code' must be a number".to_string(),
            ));
        };

        let signaled = value
            .get("signaled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(Self {
            exit_code,
            signaled,
        })
    }
}

/// Response from a terminal kill operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalKillResponse {
    /// Whether the kill operation was successful.
    pub success: bool,
}

impl TerminalKillResponse {
    /// Creates a new TerminalKillResponse.
    pub fn new(success: bool) -> Self {
        Self { success }
    }

    /// Parses a TerminalKillResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(TerminalKillResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let success = validation::require_bool_field(value, "success")?;
        Ok(Self { success })
    }
}

/// Response from a terminal release operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalReleaseResponse {
    /// Whether the release operation was successful.
    pub success: bool,
}

impl TerminalReleaseResponse {
    /// Creates a new TerminalReleaseResponse.
    pub fn new(success: bool) -> Self {
        Self { success }
    }

    /// Parses a TerminalReleaseResponse from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to parse
    ///
    /// # Returns
    ///
    /// Ok(TerminalReleaseResponse) if parsing succeeds, Error::Validation otherwise
    pub fn from_json(value: &Value) -> Result<Self> {
        let success = validation::require_bool_field(value, "success")?;
        Ok(Self { success })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_file_read_response() {
        let json = json!({"content": "test file content"});
        let response = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(response.content, "test file content");

        let json_missing = json!({"wrong_field": "value"});
        assert!(FileReadResponse::from_json(&json_missing).is_err());
    }

    #[test]
    fn test_file_write_response() {
        let json = json!({"success": true});
        let response = FileWriteResponse::from_json(&json).unwrap();
        assert_eq!(response.success, true);

        let json_false = json!({"success": false});
        let response = FileWriteResponse::from_json(&json_false).unwrap();
        assert_eq!(response.success, false);

        let json_missing = json!({"wrong_field": true});
        assert!(FileWriteResponse::from_json(&json_missing).is_err());
    }

    #[test]
    fn test_terminal_create_response() {
        let json = json!({"terminal_id": "term-123"});
        let response = TerminalCreateResponse::from_json(&json).unwrap();
        assert_eq!(response.terminal_id, "term-123");

        let json_empty = json!({"terminal_id": ""});
        let response = TerminalCreateResponse::from_json(&json_empty).unwrap();
        assert_eq!(response.terminal_id, "");

        let json_missing = json!({"wrong_field": "value"});
        assert!(TerminalCreateResponse::from_json(&json_missing).is_err());
    }

    #[test]
    fn test_terminal_output_response() {
        let json = json!({"output": "hello world", "truncated": true});
        let response = TerminalOutputResponse::from_json(&json).unwrap();
        assert_eq!(response.output, "hello world");
        assert_eq!(response.truncated, true);

        let json_no_truncate = json!({"output": "test"});
        let response = TerminalOutputResponse::from_json(&json_no_truncate).unwrap();
        assert_eq!(response.output, "test");
        assert_eq!(response.truncated, false);

        let json_missing = json!({"truncated": false});
        assert!(TerminalOutputResponse::from_json(&json_missing).is_err());
    }

    #[test]
    fn test_terminal_exit_response() {
        let json = json!({"exit_code": 0, "signaled": false});
        let response = TerminalExitResponse::from_json(&json).unwrap();
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.signaled, false);

        let json_non_zero = json!({"exit_code": 1});
        let response = TerminalExitResponse::from_json(&json_non_zero).unwrap();
        assert_eq!(response.exit_code, 1);
        assert_eq!(response.signaled, false);

        let json_negative = json!({"exit_code": -1, "signaled": true});
        let response = TerminalExitResponse::from_json(&json_negative).unwrap();
        assert_eq!(response.exit_code, -1);
        assert_eq!(response.signaled, true);

        let json_missing = json!({"signaled": true});
        assert!(TerminalExitResponse::from_json(&json_missing).is_err());
    }

    #[test]
    fn test_terminal_kill_response() {
        let json = json!({"success": true});
        let response = TerminalKillResponse::from_json(&json).unwrap();
        assert_eq!(response.success, true);

        let json_false = json!({"success": false});
        let response = TerminalKillResponse::from_json(&json_false).unwrap();
        assert_eq!(response.success, false);
    }

    #[test]
    fn test_terminal_release_response() {
        let json = json!({"success": true});
        let response = TerminalReleaseResponse::from_json(&json).unwrap();
        assert_eq!(response.success, true);

        let json_false = json!({"success": false});
        let response = TerminalReleaseResponse::from_json(&json_false).unwrap();
        assert_eq!(response.success, false);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = FileReadResponse::new("test content".to_string());
        let json = serde_json::to_value(&original).unwrap();
        let parsed = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(original, parsed);

        let original = TerminalCreateResponse::new("term-456".to_string());
        let json = serde_json::to_value(&original).unwrap();
        let parsed = TerminalCreateResponse::from_json(&json).unwrap();
        assert_eq!(original, parsed);

        let original = TerminalOutputResponse::new("output".to_string(), true);
        let json = serde_json::to_value(&original).unwrap();
        let parsed = TerminalOutputResponse::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_file_read_response_with_unicode() {
        let json = json!({"content": "Hello 世界 🌍\nMultiline\nContent"});
        let response = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(response.content, "Hello 世界 🌍\nMultiline\nContent");
    }

    #[test]
    fn test_file_read_response_with_empty_content() {
        let json = json!({"content": ""});
        let response = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(response.content, "");
    }

    #[test]
    fn test_file_read_response_with_large_content() {
        let large_content = "x".repeat(1_000_000);
        let json = json!({"content": large_content});
        let response = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(response.content.len(), 1_000_000);
    }

    #[test]
    fn test_terminal_create_response_with_special_chars() {
        let json = json!({"terminal_id": "term-123_abc@host:22"});
        let response = TerminalCreateResponse::from_json(&json).unwrap();
        assert_eq!(response.terminal_id, "term-123_abc@host:22");
    }

    #[test]
    fn test_terminal_output_response_with_ansi_codes() {
        let json = json!({
            "output": "\x1b[31mRed text\x1b[0m\n\x1b[1mBold\x1b[0m",
            "truncated": false
        });
        let response = TerminalOutputResponse::from_json(&json).unwrap();
        assert!(response.output.contains("\x1b[31m"));
        assert_eq!(response.truncated, false);
    }

    #[test]
    fn test_terminal_output_response_empty() {
        let json = json!({"output": ""});
        let response = TerminalOutputResponse::from_json(&json).unwrap();
        assert_eq!(response.output, "");
        assert_eq!(response.truncated, false);
    }

    #[test]
    fn test_terminal_exit_response_various_exit_codes() {
        // Success
        let json = json!({"exit_code": 0});
        let response = TerminalExitResponse::from_json(&json).unwrap();
        assert_eq!(response.exit_code, 0);
        assert_eq!(response.signaled, false);

        // Common error codes
        let json = json!({"exit_code": 1, "signaled": false});
        let response = TerminalExitResponse::from_json(&json).unwrap();
        assert_eq!(response.exit_code, 1);

        let json = json!({"exit_code": 127, "signaled": false});
        let response = TerminalExitResponse::from_json(&json).unwrap();
        assert_eq!(response.exit_code, 127);

        // Signal termination (e.g., SIGTERM = 143)
        let json = json!({"exit_code": 143, "signaled": true});
        let response = TerminalExitResponse::from_json(&json).unwrap();
        assert_eq!(response.exit_code, 143);
        assert_eq!(response.signaled, true);
    }

    #[test]
    fn test_terminal_exit_response_with_invalid_type() {
        // String instead of number
        let json = json!({"exit_code": "0"});
        assert!(TerminalExitResponse::from_json(&json).is_err());

        // Boolean instead of number
        let json = json!({"exit_code": true});
        assert!(TerminalExitResponse::from_json(&json).is_err());
    }

    #[test]
    fn test_response_with_extra_fields() {
        // Responses should ignore extra fields for forward compatibility
        let json = json!({
            "content": "test",
            "extra_field": "ignored",
            "another": 123
        });
        let response = FileReadResponse::from_json(&json).unwrap();
        assert_eq!(response.content, "test");

        let json = json!({
            "success": true,
            "message": "ignored",
            "timestamp": 123456
        });
        let response = FileWriteResponse::from_json(&json).unwrap();
        assert_eq!(response.success, true);
    }

    #[test]
    fn test_all_response_types_implement_required_traits() {
        // Verify all response types implement Debug, Clone, PartialEq, Serialize, Deserialize
        let file_read = FileReadResponse::new("test".to_string());
        let cloned = file_read.clone();
        assert_eq!(file_read, cloned);
        assert!(!format!("{:?}", file_read).is_empty());

        let file_write = FileWriteResponse::new(true);
        let cloned = file_write.clone();
        assert_eq!(file_write, cloned);
        assert!(!format!("{:?}", file_write).is_empty());

        let terminal_create = TerminalCreateResponse::new("term-1".to_string());
        let cloned = terminal_create.clone();
        assert_eq!(terminal_create, cloned);
        assert!(!format!("{:?}", terminal_create).is_empty());

        let terminal_output = TerminalOutputResponse::new("output".to_string(), false);
        let cloned = terminal_output.clone();
        assert_eq!(terminal_output, cloned);
        assert!(!format!("{:?}", terminal_output).is_empty());

        let terminal_exit = TerminalExitResponse::new(0, false);
        let cloned = terminal_exit.clone();
        assert_eq!(terminal_exit, cloned);
        assert!(!format!("{:?}", terminal_exit).is_empty());

        let terminal_kill = TerminalKillResponse::new(true);
        let cloned = terminal_kill.clone();
        assert_eq!(terminal_kill, cloned);
        assert!(!format!("{:?}", terminal_kill).is_empty());

        let terminal_release = TerminalReleaseResponse::new(true);
        let cloned = terminal_release.clone();
        assert_eq!(terminal_release, cloned);
        assert!(!format!("{:?}", terminal_release).is_empty());
    }
}
