//! File reading tool for MCP operations
//!
//! This module provides the ReadFileTool for reading file contents through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

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

        // Create secure file access with enhanced security validation
        let secure_access = SecureFileAccess::default_secure();

        // Perform secure read operation
        let content = secure_access.read(&request.absolute_path, request.offset, request.limit)?;

        Ok(BaseToolImpl::create_success_response(content))
    }
}
