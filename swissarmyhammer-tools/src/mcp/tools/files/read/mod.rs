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
        use serde::Deserialize;
        use crate::mcp::tools::files::shared_utils;

        #[derive(Deserialize)]
        struct ReadRequest {
            absolute_path: String,
            offset: Option<usize>,
            limit: Option<usize>,
        }

        // Parse arguments
        let request: ReadRequest = BaseToolImpl::parse_arguments(arguments)?;
        
        // Validate file path
        let validated_path = shared_utils::validate_file_path(&request.absolute_path)?;
        
        // Check if file exists
        if !shared_utils::file_exists(&validated_path)? {
            return Err(rmcp::Error::invalid_request(
                format!("File does not exist: {}", validated_path.display()),
                None,
            ));
        }

        // Read file content
        let content = std::fs::read_to_string(&validated_path)
            .map_err(|e| shared_utils::handle_file_error(e, "read", &validated_path))?;

        // Apply offset and limit if specified
        let final_content = match (request.offset, request.limit) {
            (Some(offset), Some(limit)) => {
                let lines: Vec<&str> = content.lines().collect();
                lines.iter()
                    .skip(offset.saturating_sub(1)) // Convert to 0-based index
                    .take(limit)
                    .map(|&line| line)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (Some(offset), None) => {
                let lines: Vec<&str> = content.lines().collect();
                lines.iter()
                    .skip(offset.saturating_sub(1)) // Convert to 0-based index
                    .map(|&line| line)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (None, Some(limit)) => {
                content.lines()
                    .take(limit)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (None, None) => content,
        };

        Ok(BaseToolImpl::create_success_response(final_content))
    }
}