//! Abort creation tool for MCP operations
//!
//! This module provides the AbortCreateTool for creating abort files through the MCP protocol.
//! The tool creates a `.swissarmyhammer/.abort` file containing the abort reason, enabling
//! file-based abort detection throughout the workflow system.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::Deserialize;
use std::fs;
use std::path::Path;

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

    /// Ensures the .swissarmyhammer directory exists
    fn ensure_sah_directory() -> std::result::Result<(), std::io::Error> {
        let sah_dir = Path::new(".swissarmyhammer");
        if !sah_dir.exists() {
            fs::create_dir_all(sah_dir)?;
        }
        Ok(())
    }

    /// Creates the abort file with the given reason
    fn create_abort_file(reason: &str) -> std::result::Result<(), std::io::Error> {
        Self::ensure_sah_directory()?;
        let abort_file_path = Path::new(".swissarmyhammer/.abort");
        fs::write(abort_file_path, reason)?;
        Ok(())
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

        // Create the abort file
        match Self::create_abort_file(&request.reason) {
            Ok(()) => {
                tracing::info!("Created abort file with reason: {}", request.reason);
                Ok(BaseToolImpl::create_success_response(format!(
                    "Abort file created with reason: {}",
                    request.reason
                )))
            }
            Err(e) => {
                tracing::error!("Failed to create abort file: {}", e);
                Err(McpErrorHandler::handle_error(
                    swissarmyhammer::SwissArmyHammerError::Io(e),
                    "create abort file",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::{BaseToolImpl, ToolRegistry};
    use serial_test::serial;
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
        let result: Result<AbortCreateRequest, rmcp::Error> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_create_abort_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Change to temp directory for test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_path).unwrap();

        let reason = "Test abort reason";
        let result = AbortCreateTool::create_abort_file(reason);

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        let abort_file = temp_path.join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let content = std::fs::read_to_string(&abort_file).unwrap();
        assert_eq!(content, reason);
    }

    #[test]
    #[serial]
    fn test_ensure_sah_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_path).unwrap();

        // Directory shouldn't exist initially
        let sah_dir = temp_path.join(".swissarmyhammer");
        assert!(!sah_dir.exists());

        // Should create directory
        let result = AbortCreateTool::ensure_sah_directory();

        std::env::set_current_dir(original_dir).unwrap();

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
}
