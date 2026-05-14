//! Validator-facing glob-files tool.
//!
//! This module exposes a single, focused tool named `glob_files` that performs
//! fast pattern matching across the filesystem. It is a thin wrapper around
//! [`crate::mcp::tools::files::glob::execute_glob`] — the same handler used by
//! the unified [`super::FilesTool`] when dispatched with `op: "glob files"`.
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

use super::glob;

/// Validator-facing tool that performs fast file pattern matching.
///
/// Wraps [`glob::execute_glob`] under the MCP tool name `glob_files`. Exposed
/// only on the validator endpoint; the full server uses [`super::FilesTool`]
/// for the same operation under the unified `files` name.
#[derive(Default, Debug, Clone)]
pub struct GlobFilesTool;

impl GlobFilesTool {
    /// Construct a new `GlobFilesTool`.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GlobFilesTool {
    fn name(&self) -> &'static str {
        "glob_files"
    }

    fn description(&self) -> &'static str {
        "Find files by glob pattern with advanced filtering. Supports patterns like **/*.rs and src/**/*.ts."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "description": "Find files matching a glob pattern.",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., **/*.js, src/**/*.ts)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search within (optional, defaults to current working directory)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive matching (default: false)"
                },
                "respect_git_ignore": {
                    "type": "boolean",
                    "description": "Honor .gitignore patterns (default: true)"
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
        glob::execute_glob(arguments, context).await
    }
}

impl ValidatorTool for GlobFilesTool {}

impl swissarmyhammer_common::lifecycle::Initializable for GlobFilesTool {
    fn name(&self) -> &str {
        "GlobFiles"
    }
    fn category(&self) -> &str {
        "tools"
    }
}

impl Doctorable for GlobFilesTool {
    fn name(&self) -> &str {
        "GlobFiles"
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
    fn test_name_is_glob_files() {
        let tool = GlobFilesTool::new();
        assert_eq!(<GlobFilesTool as McpTool>::name(&tool), "glob_files");
    }

    #[test]
    fn test_is_validator_tool() {
        let tool = GlobFilesTool::new();
        assert!(tool.is_validator_tool());
    }

    #[test]
    fn test_schema_requires_pattern() {
        let tool = GlobFilesTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        let required = schema["required"].as_array().expect("required array");
        assert!(required.contains(&serde_json::json!("pattern")));
    }

    #[test]
    fn test_schema_does_not_have_op_field() {
        let tool = GlobFilesTool::new();
        let schema = tool.schema();
        assert!(schema["properties"].get("op").is_none());
    }

    #[tokio::test]
    async fn test_execute_finds_matches() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.rs"), "fn alpha() {}").unwrap();
        fs::write(temp_dir.path().join("beta.txt"), "not rust").unwrap();

        let tool = GlobFilesTool::new();
        let context = create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert("pattern".to_string(), serde_json::json!("*.rs"));
        args.insert(
            "path".to_string(),
            serde_json::json!(temp_dir.path().to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_ok(),
            "glob_files should succeed on valid pattern: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_missing_pattern_errors() {
        let tool = GlobFilesTool::new();
        let context = create_test_context().await;

        let args = serde_json::Map::new();
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }
}
