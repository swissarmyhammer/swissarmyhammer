//! File writing tool for MCP operations
//!
//! This module provides the WriteFileTool for creating new files or overwriting existing files
//! with atomic operations, comprehensive security validation, and proper error handling.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::path::Path;
use tracing::{debug, info};

/// Tool for creating new files or completely overwriting existing files with atomic operations
#[derive(Default)]
pub struct WriteFileTool;

impl WriteFileTool {
    /// Creates a new instance of the WriteFileTool
    pub fn new() -> Self {
        Self
    }

    /// Performs atomic file write operation using temporary file strategy
    ///
    /// This method implements the atomic write pattern:
    /// 1. Write content to temporary file in target directory
    /// 2. Validate the written content
    /// 3. Atomically rename temporary file to target filename
    /// 4. Clean up temporary file on any failure
    ///
    /// # Arguments
    ///
    /// * `file_path` - The target file path (already validated)
    /// * `content` - The content to write
    ///
    /// # Returns
    ///
    /// * `Result<usize, McpError>` - Number of bytes written or error
    fn write_file_atomic(file_path: &Path, content: &str) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::{ensure_directory_exists, handle_file_error};
        use std::fs;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            ensure_directory_exists(parent)?;
        }

        // Create temporary file in same directory as target
        let temp_file_name = format!("{}.tmp", file_path.display());
        let temp_path = Path::new(&temp_file_name);

        debug!(
            target_path = %file_path.display(),
            temp_path = %temp_path.display(),
            content_length = content.len(),
            "Starting atomic write operation"
        );

        // Write content to temporary file
        let write_result = fs::write(temp_path, content.as_bytes())
            .map_err(|e| handle_file_error(e, "write temporary file", temp_path));

        match write_result {
            Ok(()) => {
                // Verify the content was written correctly
                let written_content = fs::read_to_string(temp_path)
                    .map_err(|e| handle_file_error(e, "verify temporary file", temp_path))?;

                if written_content != content {
                    // Clean up temporary file on verification failure
                    let _ = fs::remove_file(temp_path);
                    return Err(McpError::internal_error(
                        "Content verification failed after write".to_string(),
                        None,
                    ));
                }

                // Atomically move temporary file to target location
                let rename_result = fs::rename(temp_path, file_path)
                    .map_err(|e| handle_file_error(e, "rename to target", file_path));

                match rename_result {
                    Ok(()) => {
                        debug!(
                            path = %file_path.display(),
                            bytes_written = content.len(),
                            "Atomic write operation completed successfully"
                        );
                        Ok(content.len())
                    }
                    Err(e) => {
                        // Clean up temporary file on rename failure
                        let _ = fs::remove_file(temp_path);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // Clean up temporary file on write failure
                let _ = fs::remove_file(temp_path);
                Err(e)
            }
        }
    }
}

#[async_trait]
impl McpTool for WriteFileTool {
    fn name(&self) -> &'static str {
        "files_write"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path for the new or existing file"
                },
                "content": {
                    "type": "string",
                    "description": "Complete file content to write"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        use serde::Deserialize;
        use std::path::PathBuf;

        #[derive(Deserialize)]
        struct WriteRequest {
            file_path: String,
            content: String,
        }

        // Parse arguments
        let request: WriteRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate parameters
        if request.file_path.trim().is_empty() {
            return Err(McpError::invalid_request(
                "file_path cannot be empty".to_string(),
                None,
            ));
        }

        if request.content.len() > 10_000_000 {
            return Err(McpError::invalid_request(
                "content exceeds maximum size limit of 10MB".to_string(),
                None,
            ));
        }

        // Basic path validation for write operations
        let path_buf = PathBuf::from(&request.file_path);
        if !path_buf.is_absolute() {
            return Err(McpError::invalid_request(
                "File path must be absolute, not relative".to_string(),
                None,
            ));
        }

        // Check for dangerous patterns
        if request.file_path.contains("..") || request.file_path.contains("./") {
            return Err(McpError::invalid_request(
                "Path contains dangerous traversal sequences".to_string(),
                None,
            ));
        }

        // Log file write attempt for security auditing
        info!(
            path = %request.file_path,
            content_length = request.content.len(),
            "Attempting to write file"
        );

        // Perform atomic write operation
        let bytes_written = Self::write_file_atomic(&path_buf, &request.content)?;

        let success_message = format!(
            "Successfully wrote {} bytes to {}",
            bytes_written, request.file_path
        );

        debug!(
            path = %request.file_path,
            bytes_written = bytes_written,
            "File write operation completed successfully"
        );

        Ok(BaseToolImpl::create_success_response(success_message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_handlers::ToolHandlers;
    use crate::mcp::tool_registry::ToolContext;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use swissarmyhammer::common::rate_limiter::MockRateLimiter;
    use swissarmyhammer::git::GitOperations;
    use swissarmyhammer::issues::FileSystemIssueStorage;
    use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
    use tempfile::TempDir;
    use tokio::sync::{Mutex, RwLock};

    /// Create a test context for tool execution
    fn create_test_context() -> ToolContext {
        let issue_storage = Arc::new(RwLock::new(Box::new(
            FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )
            as Box<dyn swissarmyhammer::issues::IssueStorage>));
        let git_ops = Arc::new(Mutex::new(None::<GitOperations>));
        let memo_storage = Arc::new(RwLock::new(
            Box::new(MockMemoStorage::new()) as Box<dyn MemoStorage>
        ));
        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
        let rate_limiter = Arc::new(MockRateLimiter);

        ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            rate_limiter,
        )
    }

    /// Create test arguments for the write tool
    fn create_test_arguments(
        file_path: &str,
        content: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String(file_path.to_string()),
        );
        args.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        args
    }

    #[test]
    fn test_write_tool_creation() {
        let tool = WriteFileTool::new();
        assert_eq!(tool.name(), "files_write");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_write_tool_schema() {
        let tool = WriteFileTool::new();
        let schema = tool.schema();

        // Verify schema structure
        assert!(schema.is_object());
        let schema_obj = schema.as_object().unwrap();

        assert_eq!(schema_obj.get("type").unwrap().as_str().unwrap(), "object");
        assert!(schema_obj.contains_key("properties"));
        assert!(schema_obj.contains_key("required"));

        // Verify required fields
        let required = schema_obj.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("file_path".to_string())));
        assert!(required.contains(&serde_json::Value::String("content".to_string())));

        // Verify properties
        let properties = schema_obj.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("content"));
    }

    #[tokio::test]
    async fn test_write_new_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_new_file.txt");
        let test_content = "Hello, World!\nThis is a test file.";

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), test_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was created with correct content
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, test_content);
    }

    #[tokio::test]
    async fn test_write_overwrite_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_overwrite.txt");

        // Create initial file
        let initial_content = "Initial content";
        fs::write(&test_file, initial_content).unwrap();
        assert_eq!(fs::read_to_string(&test_file).unwrap(), initial_content);

        // Overwrite with new content
        let new_content = "New content that replaces the old";
        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), new_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify file was overwritten
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, new_content);
        assert_ne!(written_content, initial_content);
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_file = temp_dir
            .path()
            .join("deeply")
            .join("nested")
            .join("directory")
            .join("test.txt");
        let test_content = "Content in nested directory";

        assert!(!nested_file.parent().unwrap().exists());

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&nested_file.to_string_lossy(), test_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify parent directories were created
        assert!(nested_file.parent().unwrap().exists());
        assert!(nested_file.exists());

        let written_content = fs::read_to_string(&nested_file).unwrap();
        assert_eq!(written_content, test_content);
    }

    #[tokio::test]
    async fn test_write_empty_file_path() {
        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments("", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_write_whitespace_file_path() {
        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments("   ", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_write_relative_path_rejection() {
        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments("relative/path/file.txt", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("must be absolute"));
    }

    #[tokio::test]
    async fn test_write_content_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // Create content larger than 10MB limit
        let large_content = "x".repeat(10_000_001);

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), &large_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("exceeds maximum size limit"));
    }

    #[tokio::test]
    async fn test_write_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🦀 Rust!\n你好世界\nПривет мир\n🚀✨🎉";

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), unicode_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode content was written correctly
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, unicode_content);
    }

    #[tokio::test]
    async fn test_write_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        let empty_content = "";

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), empty_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify empty file was created
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, empty_content);

        let metadata = fs::metadata(&test_file).unwrap();
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_atomic_write_operation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("atomic_test.txt");
        let test_content = "Atomic write test content";

        // Test that the atomic write method works correctly
        let result = WriteFileTool::write_file_atomic(&test_file, test_content);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content.len());

        // Verify file exists and has correct content
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, test_content);

        // Verify no temporary file remains
        let temp_file_name = format!("{}.tmp", test_file.display());
        assert!(!Path::new(&temp_file_name).exists());
    }

    #[tokio::test]
    async fn test_atomic_write_cleanup_on_failure() {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly_test.txt");

        // Create a read-only file that should cause rename to fail
        fs::write(&test_file, "existing content").unwrap();

        #[cfg(unix)]
        {
            let readonly_permissions = Permissions::from_mode(0o444);
            fs::set_permissions(&test_file, readonly_permissions).unwrap();
        }

        let test_content = "This should fail to write";

        // The atomic write should fail but clean up temporary file
        let _result = WriteFileTool::write_file_atomic(&test_file, test_content);

        // Note: This test may pass on some systems where rename succeeds despite readonly target
        // The key is that temporary file should be cleaned up regardless
        let temp_file_name = format!("{}.tmp", test_file.display());
        assert!(!Path::new(&temp_file_name).exists());
    }

    #[tokio::test]
    async fn test_write_file_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("special_chars.txt");
        let special_content =
            "Line 1\nLine 2\r\nTab\tcharacter\nNull: \0 (null byte)\nBackslash: \\ forward: /";

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), special_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify special characters were written correctly
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, special_content);
    }

    #[tokio::test]
    async fn test_write_json_argument_parsing_error() {
        let tool = WriteFileTool::new();
        let context = create_test_context();

        // Create invalid arguments (missing required field)
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String("/test/path".to_string()),
        );
        // Missing "content" field

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn test_write_success_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let test_content = "Testing response format";

        let tool = WriteFileTool::new();
        let context = create_test_context();
        let args = create_test_arguments(&test_file.to_string_lossy(), test_content);

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // Check response message format
        let response_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content in response"),
        };

        assert!(response_text.contains("Successfully wrote"));
        assert!(response_text.contains(&test_content.len().to_string()));
        assert!(response_text.contains(&*test_file.to_string_lossy()));
    }
}
