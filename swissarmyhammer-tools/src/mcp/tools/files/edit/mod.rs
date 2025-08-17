//! File editing tool for MCP operations
//!
//! This module provides the EditFileTool for performing precise string replacements in files.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for performing precise string replacements in existing files
#[derive(Default)]
pub struct EditFileTool;

impl EditFileTool {
    /// Creates a new instance of the EditFileTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for EditFileTool {
    fn name(&self) -> &'static str {
        "files_edit"
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
                    "description": "Absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
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
        struct EditRequest {
            file_path: String,
            old_string: String,
            new_string: String,
            replace_all: Option<bool>,
        }

        // Parse arguments
        let request: EditRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate replacement strings are different
        if request.old_string == request.new_string {
            return Err(rmcp::Error::invalid_request(
                "old_string and new_string must be different".to_string(),
                None,
            ));
        }

        // Create secure file access with enhanced security validation
        let secure_access = SecureFileAccess::default_secure();

        // Perform secure edit operation
        let replace_all = request.replace_all.unwrap_or(false);
        secure_access.edit(
            &request.file_path,
            &request.old_string,
            &request.new_string,
            replace_all,
        )?;

        let replacements = if replace_all {
            format!("Made replacements in file: {}", request.file_path)
        } else {
            format!("Made 1 replacement in file: {}", request.file_path)
        };

        Ok(BaseToolImpl::create_success_response(replacements))
    }
}
