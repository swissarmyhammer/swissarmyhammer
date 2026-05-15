// sah rule ignore acp/capability-enforcement
//! File reading handler for MCP operations.
//!
//! This module provides [`execute_read`] — the read-file handler shared between
//! the unified [`crate::mcp::tools::files::FilesTool`] (dispatched via
//! `op: "read file"`) and the validator-facing
//! [`crate::mcp::tools::files::read_file::ReadFileTool`] (called by name).
//! It supports reading text files, binary content (with automatic base64
//! encoding), and partial reads for large files via line-based offset/limit.
//!
//! Note: This is an MCP tool, not an ACP operation. ACP capability checking happens at the
//! agent layer (claude-agent, llama-agent), not at the MCP tool layer.
//!
//! ## Features
//!
//! * **Comprehensive Security**: All file paths undergo security validation through the enhanced
//!   security framework, including workspace boundary enforcement and path traversal protection
//! * **Partial Reading**: Efficient reading of large files using line-based offset and limit
//!   parameters without loading the entire file into memory
//! * **Binary Support**: Automatic detection and base64 encoding of binary file content
//! * **Performance Optimized**: Configurable limits prevent excessive resource usage
//! * **Audit Logging**: All file access attempts are logged for security monitoring
//!
//! ## Security Considerations
//!
//! All file operations are subject to comprehensive security validation:
//! - Both absolute and relative path support with secure resolution
//! - Workspace boundary enforcement to prevent access outside authorized directories
//! - Path traversal attack prevention (blocking `../` sequences)
//! - Permission checking before file access attempts
//! - Structured audit logging for security monitoring
//!
//! ## Examples
//!
//! ```rust,ignore
//! # use swissarmyhammer_tools::mcp::tool_registry::ToolContext;
//! # use serde_json::json;
//! # async fn example(context: &ToolContext) -> Result<(), rmcp::ErrorData> {
//! use swissarmyhammer_tools::mcp::tools::files::read::execute_read;
//!
//! // Read entire file
//! let mut args = serde_json::Map::new();
//! args.insert("path".to_string(), json!("/workspace/src/main.rs"));
//! let result = execute_read(args, context).await?;
//!
//! // Read with offset and limit
//! let mut args = serde_json::Map::new();
//! args.insert("path".to_string(), json!("/workspace/logs/app.log"));
//! args.insert("offset".to_string(), json!(100));
//! args.insert("limit".to_string(), json!(50));
//! let result = execute_read(args, context).await?;
//! # Ok(())
//! # }
//! ```

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use tracing::{debug, info};

/// Operation metadata for reading files
#[derive(Debug, Default)]
pub struct ReadFile;

static READ_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("Path to the file to read (absolute or relative to current working directory)")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("offset")
        .description("Starting line number for partial reading (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("limit")
        .description("Maximum number of lines to read (optional)")
        .param_type(ParamType::Integer),
];

impl Operation for ReadFile {
    fn verb(&self) -> &'static str {
        "read"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Read file contents from the local filesystem"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        READ_FILE_PARAMS
    }
}

/// Execute a file read operation
///
/// This is the shared handler that backs both the unified
/// [`crate::mcp::tools::files::FilesTool`] (dispatched via `op: "read file"`)
/// and the validator-facing
/// [`crate::mcp::tools::files::read_file::ReadFileTool`] (called by name).
///
/// ## Security Features
///
/// * **Path Validation**: File paths (absolute or relative) undergo comprehensive security validation
/// * **Workspace Boundaries**: Enforces workspace directory restrictions to prevent unauthorized access
/// * **Path Traversal Protection**: Blocks dangerous path sequences like `../` to prevent directory traversal attacks
/// * **Permission Checking**: Validates read permissions before attempting file access
/// * **Audit Logging**: Logs all file access attempts for security monitoring and compliance
///
/// ## Performance Features
///
/// * **Configurable Limits**: Prevents excessive resource usage with offset/limit boundaries
/// * **Memory Efficient**: Supports partial reading of large files without loading entire content
/// * **Binary Support**: Automatic base64 encoding for binary files
/// * **Concurrent Safe**: Thread-safe operations for multiple simultaneous file reads
///
/// ## Supported Parameters
///
/// * `path`: Required path to the file to read (absolute or relative to current working directory)
/// * `offset`: Optional starting line number (1-based, max 1,000,000)
/// * `limit`: Optional maximum lines to read (1-100,000 lines)
pub async fn execute_read(
    arguments: serde_json::Map<String, serde_json::Value>,
    _context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    use crate::mcp::tools::files::shared_utils::SecureFileAccess;
    use serde::Deserialize;
    use swissarmyhammer_common::rate_limiter::get_rate_limiter;

    tracing::debug!(
        "files read execute() called with arguments: {:?}",
        arguments
    );

    #[derive(Deserialize)]
    struct ReadRequest {
        #[serde(alias = "absolute_path", alias = "file_path")]
        path: String,
        offset: Option<usize>,
        limit: Option<usize>,
    }

    // Parse arguments
    let request: ReadRequest = match BaseToolImpl::parse_arguments::<ReadRequest>(arguments) {
        Ok(r) => {
            tracing::debug!(
                "Parsed request successfully: path={}, offset={:?}, limit={:?}",
                r.path,
                r.offset,
                r.limit
            );
            r
        }
        Err(e) => {
            tracing::error!("Failed to parse arguments: {}", e);
            return Err(e);
        }
    };

    // Check rate limit using tokio task ID as client identifier
    let rate_limiter = get_rate_limiter();
    let client_id = format!("task_{:?}", tokio::task::try_id());
    if let Err(e) = rate_limiter.check_rate_limit(&client_id, "file_read", 1) {
        tracing::warn!("Rate limit exceeded for file_read: {}", e);
        return Err(McpError::invalid_request(
            format!("Rate limit exceeded: {}", e),
            None,
        ));
    }

    // Validate parameters before security layer
    if let Some(offset) = request.offset {
        if offset > 1_000_000 {
            return Err(McpError::invalid_request(
                "offset must be less than 1,000,000 lines".to_string(),
                None,
            ));
        }
    }

    if let Some(limit) = request.limit {
        if limit == 0 {
            return Err(McpError::invalid_request(
                "limit must be greater than 0".to_string(),
                None,
            ));
        }
        if limit > 100_000 {
            return Err(McpError::invalid_request(
                "limit must be less than or equal to 100,000 lines".to_string(),
                None,
            ));
        }
    }

    if request.path.is_empty() {
        return Err(McpError::invalid_request(
            "path cannot be empty".to_string(),
            None,
        ));
    }

    // Validate path using consistent validator approach
    let validator = FilePathValidator::default();
    let validated_path = validator.validate_path(&request.path)?;

    // Create secure file access with enhanced security validation
    let secure_access = SecureFileAccess::default_secure();

    // Log file access attempt for security auditing
    info!(
        path = %request.path,
        validated_path = %validated_path.display(),
        offset = request.offset,
        limit = request.limit,
        "Attempting to read file"
    );

    // Perform secure read operation
    let content = secure_access.read(
        &validated_path.to_string_lossy(),
        request.offset,
        request.limit,
    )?;

    debug!(
        path = %request.path,
        content_length = content.len(),
        "Successfully read file content"
    );

    Ok(BaseToolImpl::create_success_response(content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_basic_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello, world!\nLine 2\nLine 3\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Hello, world!"));
        assert!(text.contains("Line 2"));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(3));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        // Offset 3 means skip lines 1 and 2 (1-based), start from line 3
        assert!(!text.contains("Line 1"));
        assert!(!text.contains("Line 2"));
        assert!(text.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("limit_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(2));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(!text.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_limit_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(2));
        args.insert("limit".to_string(), serde_json::json!(2));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        // Offset 2 means skip line 1, start from line 2, take 2 lines
        assert!(!text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(text.contains("Line 3"));
        assert!(!text.contains("Line 4"));
    }

    #[tokio::test]
    async fn test_read_empty_path_error() {
        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert("path".to_string(), serde_json::json!(""));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("path cannot be empty") || err.contains("empty"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("does_not_exist.txt");

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(nonexistent.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            err.contains("not found") || err.contains("NotFound") || err.contains("does not exist")
        );
    }

    #[tokio::test]
    async fn test_read_offset_exceeds_max() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(1_000_001));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("offset") || err.contains("1,000,000"));
    }

    #[tokio::test]
    async fn test_read_limit_zero_error() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(0));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("limit") || err.contains("greater than 0"));
    }

    #[tokio::test]
    async fn test_read_limit_exceeds_max() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(100_001));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("limit") || err.contains("100,000"));
    }

    #[tokio::test]
    async fn test_read_file_path_alias() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("alias_test.txt");
        fs::write(&test_file, "alias test content").unwrap();

        let context = create_test_context().await;

        // Test with file_path alias
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.txt");
        fs::write(&test_file, "").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_read_unicode_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode.txt");
        let content = "Hello 🌍!\nЗдравствуй мир!\n你好世界\n";
        fs::write(&test_file, content).unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("🌍"));
        assert!(text.contains("Здравствуй"));
    }

    #[tokio::test]
    async fn test_read_missing_path_parameter() {
        let context = create_test_context().await;
        // No path field at all
        let args = serde_json::Map::new();

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
    }
}
