//! File writing tool for MCP operations
//!
//! This module provides the WriteFileTool for creating new files or overwriting existing files
//! with atomic operations, comprehensive security validation, and proper error handling.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
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
    /// 1. Write content to temporary file with unique name in target directory
    /// 2. Atomically rename temporary file to target filename
    /// 3. Clean up temporary file on any failure
    ///
    /// The temporary file uses a ULID suffix to ensure uniqueness and avoid
    /// race conditions with concurrent writes to the same file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The target file path (already validated)
    /// * `content` - The content to write
    ///
    /// # Returns
    ///
    /// * `Result<usize, McpError>` - Number of bytes written or error
    async fn write_file_atomic(file_path: &Path, content: &str) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::{ensure_directory_exists, handle_file_error};
        use tokio::fs;
        use ulid::Ulid;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            ensure_directory_exists(parent)?;
        }

        // Create temporary file with unique name in same directory as target
        let temp_file_name = format!("{}.tmp.{}", file_path.display(), Ulid::new());
        let temp_path = Path::new(&temp_file_name);

        debug!(
            target_path = %file_path.display(),
            temp_path = %temp_path.display(),
            content_length = content.len(),
            "Starting atomic write operation"
        );

        // Write content to temporary file
        let write_result = fs::write(temp_path, content.as_bytes())
            .await
            .map_err(|e| handle_file_error(e, "write temporary file", temp_path));

        match write_result {
            Ok(()) => {
                // Atomically move temporary file to target location
                let rename_result = fs::rename(temp_path, file_path)
                    .await
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
                        let _ = fs::remove_file(temp_path).await;
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // Clean up temporary file on write failure
                let _ = fs::remove_file(temp_path).await;
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

        #[derive(Deserialize)]
        struct WriteRequest {
            file_path: String,
            content: String,
        }

        // Parse arguments
        let request: WriteRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Check rate limit using tokio task ID as client identifier
        use swissarmyhammer_common::rate_limiter::get_rate_limiter;
        let rate_limiter = get_rate_limiter();
        let client_id = format!("task_{:?}", tokio::task::try_id());
        if let Err(e) = rate_limiter.check_rate_limit(&client_id, "file_write", 1) {
            tracing::warn!("Rate limit exceeded for file_write: {}", e);
            return Err(McpError::invalid_request(
                format!("Rate limit exceeded: {}", e),
                None,
            ));
        }

        // Validate parameters
        if request.file_path.trim().is_empty() {
            return Err(McpError::invalid_request(
                "file_path cannot be empty".to_string(),
                None,
            ));
        }

        const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

        if request.content.len() > MAX_FILE_SIZE {
            return Err(McpError::invalid_request(
                "content exceeds maximum size limit of 10MB".to_string(),
                None,
            ));
        }

        // First, do basic path security validation without requiring parent to exist
        use crate::mcp::tools::files::shared_utils::ensure_directory_exists;
        use std::path::PathBuf;

        // Basic path validation
        if request.file_path.trim().is_empty() {
            return Err(McpError::invalid_request(
                "File path cannot be empty".to_string(),
                None,
            ));
        }

        // Resolve to absolute path
        let path_buf = PathBuf::from(&request.file_path);
        let validated_path = if path_buf.is_absolute() {
            path_buf
        } else {
            std::env::current_dir()
                .map_err(|e| {
                    McpError::invalid_request(
                        format!("Failed to get current working directory: {}", e),
                        None,
                    )
                })?
                .join(path_buf)
        };

        // Check for path traversal attempts
        for component in validated_path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(McpError::invalid_request(
                    format!("Path traversal detected: {}", validated_path.display()),
                    None,
                ));
            }
        }

        // Ensure parent directory exists before checking permissions
        if let Some(parent) = validated_path.parent() {
            ensure_directory_exists(parent)?;
        }

        // Check write permissions after ensuring parent directory exists
        use crate::mcp::tools::files::shared_utils::{check_file_permissions, FileOperation};
        check_file_permissions(&validated_path, FileOperation::Write)?;

        // Log file write attempt for security auditing
        info!(
            path = %validated_path.display(),
            content_length = request.content.len(),
            "Attempting to write file"
        );

        // Perform atomic write operation
        let bytes_written = Self::write_file_atomic(&validated_path, &request.content).await?;

        let success_message = "OK".to_string();

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
    use std::fs;
    use tempfile::TempDir;

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
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), test_content);

        let result = tool.execute(args, &context).await;
        if let Err(e) = &result {
            eprintln!("Test failed with error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Expected write to succeed but got error: {:?}",
            result.err()
        );

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
        let context = crate::test_utils::create_test_context().await;
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
        let context = crate::test_utils::create_test_context().await;
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
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_write_whitespace_file_path() {
        let tool = WriteFileTool::new();
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("   ", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_write_relative_path_acceptance() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = WriteFileTool::new();
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("relative_file.txt", "test content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Relative paths should now be accepted");

        // Verify file was created
        let file_path = temp_dir.path().join("relative_file.txt");
        assert!(file_path.exists(), "File should have been created");

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_write_content_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // Create content larger than 10MB limit (10 * 1024 * 1024 = 10,485,760 bytes)
        let large_content = "x".repeat(10 * 1024 * 1024 + 1);

        let tool = WriteFileTool::new();
        let context = crate::test_utils::create_test_context().await;
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
        let context = crate::test_utils::create_test_context().await;
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
        let context = crate::test_utils::create_test_context().await;
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
        let result = WriteFileTool::write_file_atomic(&test_file, test_content).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content.len());

        // Verify file exists and has correct content
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, test_content);

        // Verify no temporary files remain (checking for any .tmp.* pattern)
        let parent_dir = test_file.parent().unwrap();
        let entries: Vec<_> = fs::read_dir(parent_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().contains(&format!(
                    "{}.tmp.",
                    test_file.file_name().unwrap().to_string_lossy()
                ))
            })
            .collect();
        assert!(entries.is_empty(), "Temporary files should be cleaned up");
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
        let _result = WriteFileTool::write_file_atomic(&test_file, test_content).await;

        // Note: This test may pass on some systems where rename succeeds despite readonly target
        // The key is that temporary file should be cleaned up regardless
        // Check for any .tmp.* files in the directory
        let parent_dir = test_file.parent().unwrap();
        let temp_files: Vec<_> = fs::read_dir(parent_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(
            temp_files.is_empty(),
            "Temporary files should be cleaned up after failure"
        );
    }

    #[tokio::test]
    async fn test_write_file_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("special_chars.txt");
        let special_content =
            "Line 1\nLine 2\r\nTab\tcharacter\nNull: \0 (null byte)\nBackslash: \\ forward: /";

        let tool = WriteFileTool::new();
        let context = crate::test_utils::create_test_context().await;
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
        let context = crate::test_utils::create_test_context().await;

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
        let context = crate::test_utils::create_test_context().await;
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

        assert_eq!(response_text, "OK");
    }

    #[tokio::test]
    async fn test_write_readonly_file_fails() {
        use std::fs::{self, Permissions};
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly_file.txt");

        // Create a file and make it read-only
        fs::write(&test_file, "initial content").unwrap();
        let readonly_permissions = Permissions::from_mode(0o444);
        fs::set_permissions(&test_file, readonly_permissions).unwrap();

        let tool = WriteFileTool::new();
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), "new content");

        let result = tool.execute(args, &context).await;
        assert!(result.is_err(), "Writing to read-only file should fail");

        let error = result.unwrap_err();
        let error_message = format!("{:?}", error);
        assert!(
            error_message.contains("read-only") || error_message.contains("readonly"),
            "Error should mention read-only permission: {}",
            error_message
        );
    }
}
