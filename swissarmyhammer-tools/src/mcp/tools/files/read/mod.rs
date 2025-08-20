//! File reading tool for MCP operations
//!
//! This module provides the `ReadFileTool` for secure, validated file reading operations through
//! the MCP protocol. The tool supports reading various file types including text files, binary
//! content (with base64 encoding), and provides partial reading capabilities for large files.
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
//! - Absolute path requirements to prevent relative path confusion
//! - Workspace boundary enforcement to prevent access outside authorized directories
//! - Path traversal attack prevention (blocking `../` sequences)
//! - Permission checking before file access attempts
//! - Structured audit logging for security monitoring
//!
//! ## Examples
//!
//! ```rust,no_run
//! # use swissarmyhammer_tools::mcp::tools::files::read::ReadFileTool;
//! # use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
//! # use serde_json::json;
//! # async fn example(context: &ToolContext) -> Result<(), rmcp::Error> {
//! let tool = ReadFileTool::new();
//!
//! // Read entire file
//! let mut args = serde_json::Map::new();
//! args.insert("absolute_path".to_string(), json!("/workspace/src/main.rs"));
//! let result = tool.execute(args, context).await?;
//!
//! // Read with offset and limit
//! let mut args = serde_json::Map::new();
//! args.insert("absolute_path".to_string(), json!("/workspace/logs/app.log"));
//! args.insert("offset".to_string(), json!(100));
//! args.insert("limit".to_string(), json!(50));
//! let result = tool.execute(args, context).await?;
//! # Ok(())
//! # }
//! ```

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use tracing::{debug, info};

/// Tool for reading file contents from the local filesystem with comprehensive security validation
///
/// `ReadFileTool` provides secure, validated file reading operations through the MCP protocol.
/// It supports reading various file types, partial reading for large files, and implements
/// comprehensive security measures including workspace boundary enforcement and path validation.
///
/// ## Security Features
///
/// * **Path Validation**: All file paths must be absolute and undergo comprehensive security validation
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
/// * `absolute_path`: Required absolute path to the file to read
/// * `offset`: Optional starting line number (1-based, max 1,000,000)
/// * `limit`: Optional maximum lines to read (1-100,000 lines)
///
/// ## Usage Examples
///
/// ```rust,no_run
/// use swissarmyhammer_tools::mcp::tools::files::read::ReadFileTool;
/// use swissarmyhammer_tools::mcp::tool_registry::McpTool;
///
/// let tool = ReadFileTool::new();
/// assert_eq!(tool.name(), "files_read");
/// ```
#[derive(Default, Debug, Clone)]
pub struct ReadFileTool;

impl ReadFileTool {
    /// Creates a new instance of the ReadFileTool
    ///
    /// Returns a new `ReadFileTool` instance ready for file reading operations.
    /// The tool is lightweight and can be cloned efficiently for concurrent usage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swissarmyhammer_tools::mcp::tools::files::read::ReadFileTool;
    /// use swissarmyhammer_tools::mcp::tool_registry::McpTool;
    ///
    /// let tool = ReadFileTool::new();
    /// assert_eq!(tool.name(), "files_read");
    /// ```
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ReadFileTool {
    fn name(&self) -> &'static str {
        "files_read"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "absolute_path": {
                    "type": "string",
                    "description": "Full absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Starting line number for partial reading (optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (optional)"
                }
            },
            "required": ["absolute_path"]
        })
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("file")
    }

    fn cli_name(&self) -> &'static str {
        "read"
    }

    fn cli_about(&self) -> Option<&'static str> {
        Some("Read file contents with optional offset and limit")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        use crate::mcp::tools::files::shared_utils::SecureFileAccess;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct ReadRequest {
            absolute_path: String,
            offset: Option<usize>,
            limit: Option<usize>,
        }

        // Parse arguments
        let request: ReadRequest = BaseToolImpl::parse_arguments(arguments)?;

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

        if request.absolute_path.is_empty() {
            return Err(McpError::invalid_request(
                "absolute_path cannot be empty".to_string(),
                None,
            ));
        }

        // Create secure file access with enhanced security validation
        let secure_access = SecureFileAccess::default_secure();

        // Log file access attempt for security auditing
        info!(
            path = %request.absolute_path,
            offset = request.offset,
            limit = request.limit,
            "Attempting to read file"
        );

        // Perform secure read operation
        let content = secure_access.read(&request.absolute_path, request.offset, request.limit)?;

        debug!(
            path = %request.absolute_path,
            content_length = content.len(),
            "Successfully read file content"
        );

        Ok(BaseToolImpl::create_success_response(content))
    }
}
