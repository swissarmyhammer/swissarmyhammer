//! Validator-facing read-file tool.
//!
//! This module exposes a single, focused tool named `read_file` that reads file
//! contents from the local filesystem. It is a thin wrapper around
//! [`crate::mcp::tools::files::read::execute_read`] — the same handler used by
//! the unified [`super::FilesTool`] when dispatched with `op: "read file"`.
//!
//! ## Why this exists
//!
//! Models trained on Hermes-style tool schemas (e.g. the Qwen3 family) call
//! tools by **name**. They naturally emit:
//!
//! ```text
//! <tool_call>{"name": "read_file", "arguments": {"path": "..."}}</tool_call>
//! ```
//!
//! The unified `files` tool with an `op` argument is a CLI convenience and
//! does not match the natural shape models emit. The validator MCP endpoint
//! therefore exposes the per-operation tools (`read_file`, `glob_files`,
//! `grep_files`) so validators can call them by name with no
//! prompt-engineering glue.
//!
//! ## Scope
//!
//! - **Validator-only**: `is_validator_tool()` returns `true`. The full server
//!   continues to expose the unified `files` tool for non-validator agents.
//! - **No write operations**: This tool wraps only the read handler — write
//!   and edit operations are not reachable through it.

use crate::mcp::tool_registry::{McpTool, ToolContext, ValidatorTool};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};

use super::read;

/// Validator-facing tool that reads file contents from the local filesystem.
///
/// Wraps [`read::execute_read`] under the MCP tool name `read_file`. Exposed
/// only on the validator endpoint; the full server uses [`super::FilesTool`]
/// for the same operation under the unified `files` name.
#[derive(Default, Debug, Clone)]
pub struct ReadFileTool;

impl ReadFileTool {
    /// Construct a new `ReadFileTool`.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read file contents from the local filesystem. Returns text for text files; binary files are returned as base64. Supports optional line-based offset and limit for partial reads."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "description": "Read file contents from the local filesystem.",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (absolute or relative to current working directory)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Starting line number for partial reading (1-based, max 1,000,000)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (1 to 100,000)"
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn cli_category(&self) -> Option<&'static str> {
        // Validator-only tool; no CLI command surface.
        None
    }

    fn hidden_from_cli(&self) -> bool {
        true
    }

    fn is_validator_tool(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        read::execute_read(arguments, context).await
    }
}

impl ValidatorTool for ReadFileTool {}

impl swissarmyhammer_common::lifecycle::Initializable for ReadFileTool {
    fn name(&self) -> &str {
        "ReadFile"
    }
    fn category(&self) -> &str {
        "tools"
    }
}

impl Doctorable for ReadFileTool {
    fn name(&self) -> &str {
        "ReadFile"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        Vec::new()
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_name_is_read_file() {
        let tool = ReadFileTool::new();
        assert_eq!(<ReadFileTool as McpTool>::name(&tool), "read_file");
    }

    #[test]
    fn test_is_validator_tool() {
        let tool = ReadFileTool::new();
        assert!(tool.is_validator_tool());
    }

    #[test]
    fn test_is_not_agent_tool() {
        let tool = ReadFileTool::new();
        // Read-only validator tools are not agent tools.
        assert!(!tool.is_agent_tool());
    }

    #[test]
    fn test_schema_requires_path() {
        let tool = ReadFileTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
        let required = schema["required"].as_array().expect("required array");
        assert!(required.contains(&serde_json::json!("path")));
    }

    #[test]
    fn test_schema_does_not_have_op_field() {
        // The split tool's surface is its name — no `op` argument.
        let tool = ReadFileTool::new();
        let schema = tool.schema();
        assert!(schema["properties"].get("op").is_none());
    }

    #[tokio::test]
    async fn test_execute_reads_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("hello.txt");
        fs::write(&test_file, "hello world").unwrap();

        let tool = ReadFileTool::new();
        let context = create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_ok(),
            "read_file should succeed on valid path: {:?}",
            result.err()
        );
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("hello world"));
    }

    #[tokio::test]
    async fn test_execute_missing_path_errors() {
        let tool = ReadFileTool::new();
        let context = create_test_context().await;

        let args = serde_json::Map::new();
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_nonexistent_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("does_not_exist.txt");

        let tool = ReadFileTool::new();
        let context = create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(missing.to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }
}
