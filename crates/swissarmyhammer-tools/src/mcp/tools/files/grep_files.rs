//! Validator-facing grep-files tool.
//!
//! This module exposes a single, focused tool named `grep_files` that performs
//! ripgrep-backed content search. It is a thin wrapper around
//! [`crate::mcp::tools::files::grep::execute_grep`] — the same handler used by
//! the unified [`super::FilesTool`] when dispatched with `op: "grep files"`.
//!
//! Exists for the same reason as [`super::read_file::ReadFileTool`]: the
//! validator endpoint needs tools whose **names** match what Hermes-style
//! models naturally emit, rather than the CLI-friendly `op`-dispatched form.

use crate::mcp::tool_registry::{McpTool, ToolContext, ValidatorTool};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};

use super::grep;

/// Validator-facing tool that performs ripgrep-style content search.
///
/// Wraps [`grep::execute_grep`] under the MCP tool name `grep_files`. Exposed
/// only on the validator endpoint; the full server uses [`super::FilesTool`]
/// for the same operation under the unified `files` name.
#[derive(Default, Debug, Clone)]
pub struct GrepFilesTool;

impl GrepFilesTool {
    /// Construct a new `GrepFilesTool`.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GrepFilesTool {
    fn name(&self) -> &'static str {
        "grep_files"
    }

    fn description(&self) -> &'static str {
        "Search file contents with a regular expression using ripgrep. Optionally filter by glob or file type."
    }

    fn schema(&self) -> serde_json::Value {
        // NOTE: `context_lines` is intentionally omitted here even though the
        // underlying `GrepRequest` accepts it. The handler does not honor it
        // (see the `#[allow(dead_code)]` on `GrepRequest::context_lines` in
        // `grep/mod.rs`), and advertising an unimplemented capability would
        // mislead Hermes-trained validator models into emitting calls that
        // silently produce no context lines with no error to recover from.
        // Re-add this field only after `execute_grep` actually returns
        // surrounding context lines.
        json!({
            "type": "object",
            "description": "Search file contents with a regular expression.",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (optional, defaults to current working directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., *.js)"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter (e.g., js, py, rust)"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search"
                },
                "output_mode": {
                    "type": "string",
                    "description": "Output format: content, files_with_matches, or count"
                }
            },
            "required": ["pattern"],
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
        grep::execute_grep(arguments, context).await
    }
}

impl ValidatorTool for GrepFilesTool {}

impl swissarmyhammer_common::lifecycle::Initializable for GrepFilesTool {
    fn name(&self) -> &str {
        "GrepFiles"
    }
    fn category(&self) -> &str {
        "tools"
    }
}

impl Doctorable for GrepFilesTool {
    fn name(&self) -> &str {
        "GrepFiles"
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
    fn test_name_is_grep_files() {
        let tool = GrepFilesTool::new();
        assert_eq!(<GrepFilesTool as McpTool>::name(&tool), "grep_files");
    }

    #[test]
    fn test_is_validator_tool() {
        let tool = GrepFilesTool::new();
        assert!(tool.is_validator_tool());
    }

    #[test]
    fn test_schema_requires_pattern() {
        let tool = GrepFilesTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        let required = schema["required"].as_array().expect("required array");
        assert!(required.contains(&serde_json::json!("pattern")));
    }

    #[test]
    fn test_schema_does_not_have_op_field() {
        let tool = GrepFilesTool::new();
        let schema = tool.schema();
        assert!(schema["properties"].get("op").is_none());
    }

    /// `context_lines` must not appear in the schema. The underlying
    /// `execute_grep` does not honor it (the field on `GrepRequest` is marked
    /// `#[allow(dead_code)]`), so advertising it would mislead validator models
    /// that pass `{"context_lines": N}` and silently get no surrounding context.
    #[test]
    fn test_schema_does_not_have_context_lines() {
        let tool = GrepFilesTool::new();
        let schema = tool.schema();
        assert!(
            schema["properties"].get("context_lines").is_none(),
            "context_lines must not be advertised until execute_grep honors it"
        );
    }

    #[tokio::test]
    async fn test_execute_finds_matches() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("greeting.txt"),
            "hello world\nfoo bar\n",
        )
        .unwrap();

        let tool = GrepFilesTool::new();
        let context = create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert("pattern".to_string(), serde_json::json!("hello"));
        args.insert(
            "path".to_string(),
            serde_json::json!(temp_dir.path().to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_ok(),
            "grep_files should succeed on valid pattern: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_missing_pattern_errors() {
        let tool = GrepFilesTool::new();
        let context = create_test_context().await;

        let args = serde_json::Map::new();
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }
}
