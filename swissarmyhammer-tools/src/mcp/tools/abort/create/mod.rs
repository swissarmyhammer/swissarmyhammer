//! Abort creation tool for MCP operations
//!
//! This module provides the AbortCreateTool for creating abort files through the MCP protocol.
//! The tool creates a `.swissarmyhammer/.abort` file containing the abort reason, enabling
//! file-based abort detection throughout the workflow system.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use swissarmyhammer_common::create_abort_file_current_dir;

/// Request structure for creating an abort file
#[derive(Debug, Deserialize)]
pub struct AbortCreateRequest {
    /// Reason for the abort (required)
    pub reason: String,
}

/// Tool for creating abort files to signal workflow termination
#[derive(Default)]
pub struct AbortCreateTool;

impl AbortCreateTool {
    /// Creates a new instance of the AbortCreateTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for AbortCreateTool {
    fn name(&self) -> &'static str {
        "abort_create"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Reason for the abort (required)"
                }
            },
            "required": ["reason"]
        })
    }

    fn hidden_from_cli(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for abort creation
        context
            .rate_limiter
            .check_rate_limit("unknown", "abort_create", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for abort creation: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Creating abort with reason: {}", request.reason);

        // Validate reason is not empty
        McpValidation::validate_not_empty(&request.reason, "abort reason")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate abort reason"))?;

        // Create the abort file using shared utility (will panic if it fails)
        create_abort_file_current_dir(&request.reason);

        tracing::info!("Created abort file with reason: {}", request.reason);
        Ok(BaseToolImpl::create_success_response(format!(
            "Abort file created with reason: {}",
            request.reason
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::{BaseToolImpl, ToolRegistry};
    use swissarmyhammer::test_utils::IsolatedTestHome;
    use swissarmyhammer_common::create_abort_file;
    use tempfile::TempDir;

    #[test]
    fn test_abort_create_tool_name() {
        let tool = AbortCreateTool::new();
        assert_eq!(tool.name(), "abort_create");
    }

    #[test]
    fn test_abort_create_tool_description() {
        let tool = AbortCreateTool::new();
        let description = tool.description();
        assert!(description.contains("abort file"));
        assert!(description.contains("reason"));
    }

    #[test]
    fn test_abort_create_tool_schema() {
        let tool = AbortCreateTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("reason"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("reason".to_string())));
    }

    #[test]
    fn test_parse_valid_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "reason".to_string(),
            serde_json::Value::String("Test abort reason".to_string()),
        );

        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.reason, "Test abort reason");
    }

    #[test]
    fn test_parse_missing_reason() {
        let args = serde_json::Map::new();
        let result: Result<AbortCreateRequest, rmcp::ErrorData> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_abort_file() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let reason = "Test abort reason";
        let result = create_abort_file(temp_path, reason);

        assert!(result.is_ok());

        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, reason);
    }

    #[test]
    fn test_creates_sah_directory() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Directory shouldn't exist initially
        let sah_dir = temp_path.join(".swissarmyhammer");
        assert!(!sah_dir.exists());

        // Should create directory when creating abort file
        let result = create_abort_file(temp_path, "test");

        assert!(result.is_ok());
        assert!(sah_dir.exists());
        assert!(sah_dir.is_dir());
    }

    #[test]
    fn test_tool_registration() {
        let mut registry = ToolRegistry::new();
        registry.register(AbortCreateTool::new());

        assert!(registry.get_tool("abort_create").is_some());
        assert!(registry
            .list_tool_names()
            .contains(&"abort_create".to_string()));
    }

    #[test]
    fn test_concurrent_abort_file_creation() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create multiple threads trying to create abort file simultaneously
        let temp_path = temp_path.to_path_buf();
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let path = temp_path.clone();
                std::thread::spawn(move || {
                    let reason = format!("Concurrent abort reason {i}");
                    create_abort_file(&path, &reason)
                })
            })
            .collect();

        // Wait for all threads to complete
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should succeed
        for result in results {
            assert!(result.is_ok());
        }

        // File should exist
        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());
    }

    #[test]
    fn test_abort_file_overwrite() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create first abort file
        let first_reason = "First abort reason";
        let result1 = create_abort_file(temp_path, first_reason);
        assert!(result1.is_ok());

        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let content1 = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content1, first_reason);

        // Create second abort file (should overwrite)
        let second_reason = "Second abort reason";
        let result2 = create_abort_file(temp_path, second_reason);
        assert!(result2.is_ok());

        let content2 = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content2, second_reason);
    }

    #[test]
    fn test_parse_empty_reason() {
        let mut args = serde_json::Map::new();
        args.insert(
            "reason".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.reason, "");
    }

    #[test]
    fn test_parse_whitespace_only_reason() {
        let mut args = serde_json::Map::new();
        args.insert(
            "reason".to_string(),
            serde_json::Value::String("   \t\n  ".to_string()),
        );

        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.reason, "   \t\n  ");
    }

    #[test]
    fn test_parse_long_reason() {
        let long_reason = "x".repeat(10000);
        let mut args = serde_json::Map::new();
        args.insert(
            "reason".to_string(),
            serde_json::Value::String(long_reason.clone()),
        );

        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.reason, long_reason);
    }

    #[test]
    fn test_parse_unicode_reason() {
        let unicode_reason = "中文测试 🚫 Aborting with émojis and ñoñ-ASCII";
        let mut args = serde_json::Map::new();
        args.insert(
            "reason".to_string(),
            serde_json::Value::String(unicode_reason.to_string()),
        );

        let request: AbortCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.reason, unicode_reason);
    }

    #[test]
    fn test_parse_invalid_type() {
        let mut args = serde_json::Map::new();
        args.insert("reason".to_string(), serde_json::Value::Number(42.into()));

        let result: Result<AbortCreateRequest, rmcp::ErrorData> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_abort_file_with_unicode() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let unicode_reason = "Abort with émojis 🚫 and ñoñ-ASCII characters 中文";
        let result = create_abort_file(temp_path, unicode_reason);

        assert!(result.is_ok());

        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, unicode_reason);
    }

    #[test]
    fn test_create_abort_file_when_directory_already_exists() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Pre-create the .swissarmyhammer directory
        let sah_dir = temp_path.join(".swissarmyhammer");
        std::fs::create_dir(&sah_dir).unwrap();
        assert!(sah_dir.exists());

        let reason = "Test with existing directory";
        let result = create_abort_file(temp_path, reason);

        assert!(result.is_ok());

        let abort_file = sah_dir.join(".abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, reason);
    }

    #[test]
    fn test_abort_file_operations_idempotent() {
        let _guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let sah_dir = temp_path.join(".swissarmyhammer");

        // Multiple calls to create_abort_file should work
        let result1 = create_abort_file(temp_path, "first reason");
        assert!(result1.is_ok());
        assert!(sah_dir.exists());

        let result2 = create_abort_file(temp_path, "second reason");
        assert!(result2.is_ok());
        assert!(sah_dir.exists());

        // The second reason should overwrite the first
        let abort_file = sah_dir.join(".abort");
        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, "second reason");
    }
}
