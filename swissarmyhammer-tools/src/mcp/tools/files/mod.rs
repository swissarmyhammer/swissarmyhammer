//! Unified file operations tool for MCP
//!
//! This module provides a single `files` tool that dispatches between operations:
//! - `read file`: Read file contents from the local filesystem
//! - `write file`: Create new files or overwrite existing files
//! - `edit file`: Perform precise string replacements in existing files
//! - `glob files`: Fast file pattern matching with advanced filtering
//! - `grep files`: Content-based search using ripgrep
//!
//! Follows the Operation pattern from `swissarmyhammer-operations`.

pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod schema;
pub mod shared_utils;
pub mod write;

use crate::mcp::tool_registry::{AgentTool, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::health::{Doctorable, HealthCheck};
use swissarmyhammer_operations::Operation;

use edit::EditFile;
use glob::GlobFiles;
use grep::GrepFiles;
use read::ReadFile;
use write::WriteFile;

// Static operation instances for schema generation
static READ_FILE: Lazy<ReadFile> = Lazy::new(ReadFile::default);
static WRITE_FILE: Lazy<WriteFile> = Lazy::new(WriteFile::default);
static EDIT_FILE: Lazy<EditFile> = Lazy::new(EditFile::default);
static GLOB_FILES: Lazy<GlobFiles> = Lazy::new(GlobFiles::default);
static GREP_FILES: Lazy<GrepFiles> = Lazy::new(GrepFiles::default);

static FILE_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*READ_FILE as &dyn Operation,
        &*WRITE_FILE as &dyn Operation,
        &*EDIT_FILE as &dyn Operation,
        &*GLOB_FILES as &dyn Operation,
        &*GREP_FILES as &dyn Operation,
    ]
});

/// Unified file operations tool providing read, write, edit, glob, and grep
#[derive(Default)]
pub struct FilesTool;

impl FilesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for FilesTool {
    fn name(&self) -> &'static str {
        "files"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_files_mcp_schema(&FILE_OPERATIONS)
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("files")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip the "op" key from arguments before passing to handlers
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "read file" => read::execute_read(args, context).await,
            "write file" => write::execute_write(args, context).await,
            "edit file" => edit::execute_edit(args, context).await,
            "glob files" => glob::execute_glob(args, context).await,
            "grep files" => grep::execute_grep(args, context).await,
            "" => {
                // Infer operation from present keys
                if arguments.contains_key("old_string") || arguments.contains_key("new_string") {
                    edit::execute_edit(args, context).await
                } else if arguments.contains_key("content") {
                    write::execute_write(args, context).await
                } else if arguments.contains_key("pattern") && arguments.contains_key("case_insensitive") {
                    grep::execute_grep(args, context).await
                } else if arguments.contains_key("pattern") {
                    glob::execute_glob(args, context).await
                } else if arguments.contains_key("path") {
                    read::execute_read(args, context).await
                } else {
                    Err(McpError::invalid_params(
                        "Cannot determine operation. Provide 'op' field (\"read file\", \"write file\", \"edit file\", \"glob files\", or \"grep files\").",
                        None,
                    ))
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'read file', 'write file', 'edit file', 'glob files', 'grep files'",
                    other
                ),
                None,
            )),
        }
    }
}

#[async_trait]
impl AgentTool for FilesTool {}

impl Doctorable for FilesTool {
    fn name(&self) -> &str {
        "Files"
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

/// Register the unified files tool with the registry
pub async fn register_file_tools(registry: &mut ToolRegistry) {
    registry.register(FilesTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[tokio::test]
    async fn test_register_file_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_file_tools(&mut registry).await;

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("files").is_some());
    }

    #[test]
    fn test_files_tool_name() {
        let tool = FilesTool::new();
        assert_eq!(<FilesTool as McpTool>::name(&tool), "files");
    }

    #[test]
    fn test_files_tool_has_description() {
        let tool = FilesTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_files_tool_schema_has_op_field() {
        let tool = FilesTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("read file")));
        assert!(op_enum.contains(&serde_json::json!("write file")));
        assert!(op_enum.contains(&serde_json::json!("edit file")));
        assert!(op_enum.contains(&serde_json::json!("glob files")));
        assert!(op_enum.contains(&serde_json::json!("grep files")));
    }

    #[test]
    fn test_files_tool_schema_has_operation_schemas() {
        let tool = FilesTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 5);
    }

    #[tokio::test]
    async fn test_files_tool_unknown_op() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "op".to_string(),
            serde_json::Value::String("invalid op".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_files_tool_missing_op_and_no_keys() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let args = serde_json::Map::new();

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot determine operation"));
    }

    #[tokio::test]
    async fn test_files_tool_dispatch_read() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        // Create a temp file to read
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("read file"));
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_files_tool_dispatch_write() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("write_test.txt");

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("write file"));
        args.insert(
            "file_path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("content".to_string(), serde_json::json!("test content"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
        assert!(test_file.exists());
    }

    #[tokio::test]
    async fn test_files_tool_dispatch_edit() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("edit_test.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("edit file"));
        args.insert(
            "file_path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("old_string".to_string(), serde_json::json!("hello"));
        args.insert("new_string".to_string(), serde_json::json!("goodbye"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "goodbye world"
        );
    }

    #[tokio::test]
    async fn test_files_tool_dispatch_glob() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("glob files"));
        args.insert("pattern".to_string(), serde_json::json!("*.rs"));
        args.insert(
            "path".to_string(),
            serde_json::json!(temp_dir.path().to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_files_tool_dispatch_grep() {
        let tool = FilesTool::new();
        let context = crate::test_utils::create_test_context().await;

        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("test.txt"), "hello world\nfoo bar").unwrap();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("grep files"));
        args.insert("pattern".to_string(), serde_json::json!("hello"));
        args.insert(
            "path".to_string(),
            serde_json::json!(temp_dir.path().to_string_lossy()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }
}
