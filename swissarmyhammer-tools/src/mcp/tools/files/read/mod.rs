//! File reading tool for MCP operations
//!
//! This module provides the ReadFileTool for reading file contents through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use tracing::{debug, info};

/// Tool for reading file contents from the local filesystem
#[derive(Default)]
pub struct ReadFileTool;

impl ReadFileTool {
    /// Creates a new instance of the ReadFileTool
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
                    "type": "number",
                    "description": "Starting line number for partial reading (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to read (optional)"
                }
            },
            "required": ["absolute_path"]
        })
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
