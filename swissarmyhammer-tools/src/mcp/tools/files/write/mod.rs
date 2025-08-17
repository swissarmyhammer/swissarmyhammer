//! File writing tool for MCP operations
//!
//! This module provides the WriteFileTool for creating new files or overwriting existing files.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for creating new files or completely overwriting existing files
#[derive(Default)]
pub struct WriteFileTool;

impl WriteFileTool {
    /// Creates a new instance of the WriteFileTool
    pub fn new() -> Self {
        Self
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
        use crate::mcp::tools::files::shared_utils::SecureFileAccess;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct WriteRequest {
            file_path: String,
            content: String,
        }

        // Parse arguments
        let request: WriteRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Create secure file access with enhanced security validation
        let secure_access = SecureFileAccess::default_secure();
        
        // Perform secure write operation
        secure_access.write(&request.file_path, &request.content)?;

        let success_message = format!("File written successfully: {}", request.file_path);
        Ok(BaseToolImpl::create_success_response(success_message))
    }
}
