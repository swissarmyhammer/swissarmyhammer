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

use crate::mcp::tool_registry::{AgentTool, McpTool, ToolContext, ToolRegistry, ValidatorTool};
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

static READ_ONLY_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*READ_FILE as &dyn Operation,
        &*GLOB_FILES as &dyn Operation,
        &*GREP_FILES as &dyn Operation,
    ]
});

/// Defines which file operations are available
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperationSubset {
    /// All operations (read, write, edit, glob, grep) — default behavior
    All,
    /// Read-only operations (read, glob, grep) — for validators
    ReadOnly,
}

/// Unified file operations tool providing read, write, edit, glob, and grep.
///
/// Can be configured with an operation subset to restrict available operations.
pub struct FilesTool {
    operations: FileOperationSubset,
}

impl Default for FilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FilesTool {
    /// Create a FilesTool with all operations (default, backward compatible)
    pub fn new() -> Self {
        Self {
            operations: FileOperationSubset::All,
        }
    }

    /// Create a read-only FilesTool (read, glob, grep only) for validators
    pub fn read_only() -> Self {
        Self {
            operations: FileOperationSubset::ReadOnly,
        }
    }
}

#[async_trait]
impl McpTool for FilesTool {
    fn name(&self) -> &'static str {
        "files"
    }

    fn description(&self) -> &'static str {
        match self.operations {
            FileOperationSubset::All => include_str!("description.md"),
            FileOperationSubset::ReadOnly => "Read-only file operations: read file contents, glob for file patterns, and grep for content search. Write and edit operations are not available.",
        }
    }

    fn schema(&self) -> serde_json::Value {
        match self.operations {
            FileOperationSubset::All => schema::generate_files_mcp_schema(&FILE_OPERATIONS),
            FileOperationSubset::ReadOnly => {
                schema::generate_files_mcp_schema(&READ_ONLY_OPERATIONS)
            }
        }
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("files")
    }

    fn is_agent_tool(&self) -> bool {
        // Read-only variant is NOT agent-only — validators need it even without agent mode
        self.operations == FileOperationSubset::All
    }

    fn is_validator_tool(&self) -> bool {
        // Only the read-only variant is available to validators
        self.operations == FileOperationSubset::ReadOnly
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

        // Reject disallowed operations in read-only mode
        if self.operations == FileOperationSubset::ReadOnly {
            match op_str {
                "write file" | "edit file" => {
                    return Err(McpError::invalid_params(
                        format!(
                            "Operation '{}' is not available in read-only mode. Only 'read file', 'glob files', and 'grep files' are available.",
                            op_str
                        ),
                        None,
                    ));
                }
                _ => {}
            }
        }

        match op_str {
            "read file" => read::execute_read(args, context).await,
            "write file" => write::execute_write(args, context).await,
            "edit file" => edit::execute_edit(args, context).await,
            "glob files" => glob::execute_glob(args, context).await,
            "grep files" => grep::execute_grep(args, context).await,
            "" => {
                // Infer operation from present keys
                if self.operations == FileOperationSubset::ReadOnly {
                    // In read-only mode, only infer read operations
                    if arguments.contains_key("pattern")
                        && arguments.contains_key("case_insensitive")
                    {
                        grep::execute_grep(args, context).await
                    } else if arguments.contains_key("pattern") {
                        glob::execute_glob(args, context).await
                    } else if arguments.contains_key("path") {
                        read::execute_read(args, context).await
                    } else {
                        Err(McpError::invalid_params(
                            "Cannot determine operation. Provide 'op' field (\"read file\", \"glob files\", or \"grep files\").",
                            None,
                        ))
                    }
                } else if arguments.contains_key("old_string")
                    || arguments.contains_key("new_string")
                {
                    edit::execute_edit(args, context).await
                } else if arguments.contains_key("content") {
                    write::execute_write(args, context).await
                } else if arguments.contains_key("pattern")
                    && arguments.contains_key("case_insensitive")
                {
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
                    "Unknown operation '{}'. Valid operations: {}",
                    other,
                    if self.operations == FileOperationSubset::ReadOnly {
                        "'read file', 'glob files', 'grep files'"
                    } else {
                        "'read file', 'write file', 'edit file', 'glob files', 'grep files'"
                    }
                ),
                None,
            )),
        }
    }
}

#[async_trait]
impl AgentTool for FilesTool {}

impl ValidatorTool for FilesTool {}

impl swissarmyhammer_common::lifecycle::Initializable for FilesTool {
    fn name(&self) -> &str {
        "Files"
    }
    fn category(&self) -> &str {
        "tools"
    }
}

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

    // =========================================================================
    // Read-only mode tests
    // =========================================================================

    #[test]
    fn test_read_only_schema_has_3_ops() {
        let tool = FilesTool::read_only();
        let schema = tool.schema();

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 3);
        assert!(op_enum.contains(&serde_json::json!("read file")));
        assert!(op_enum.contains(&serde_json::json!("glob files")));
        assert!(op_enum.contains(&serde_json::json!("grep files")));
        assert!(!op_enum.contains(&serde_json::json!("write file")));
        assert!(!op_enum.contains(&serde_json::json!("edit file")));
    }

    #[test]
    fn test_read_only_is_not_agent_tool() {
        let tool = FilesTool::read_only();
        assert!(!tool.is_agent_tool());
    }

    #[test]
    fn test_all_is_agent_tool() {
        let tool = FilesTool::new();
        assert!(tool.is_agent_tool());
    }

    #[tokio::test]
    async fn test_read_only_rejects_write() {
        let tool = FilesTool::read_only();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("write file"));
        args.insert("file_path".to_string(), serde_json::json!("/tmp/test.txt"));
        args.insert("content".to_string(), serde_json::json!("test"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not available in read-only mode"));
    }

    #[tokio::test]
    async fn test_read_only_rejects_edit() {
        let tool = FilesTool::read_only();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("edit file"));
        args.insert("file_path".to_string(), serde_json::json!("/tmp/test.txt"));
        args.insert("old_string".to_string(), serde_json::json!("old"));
        args.insert("new_string".to_string(), serde_json::json!("new"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not available in read-only mode"));
    }

    #[tokio::test]
    async fn test_read_only_allows_read() {
        let tool = FilesTool::read_only();
        let context = crate::test_utils::create_test_context().await;

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
    async fn test_read_only_allows_glob() {
        let tool = FilesTool::read_only();
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
}
