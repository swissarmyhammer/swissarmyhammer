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
        use crate::mcp::tools::files::shared_utils;
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

        // Validate file path
        let validated_path = shared_utils::validate_file_path(&request.file_path)?;

        // Check if file exists
        if !shared_utils::file_exists(&validated_path)? {
            return Err(rmcp::Error::invalid_request(
                format!("File does not exist: {}", validated_path.display()),
                None,
            ));
        }

        // Validate replacement strings are different
        if request.old_string == request.new_string {
            return Err(rmcp::Error::invalid_request(
                "old_string and new_string must be different".to_string(),
                None,
            ));
        }

        // Read current file content
        let content = std::fs::read_to_string(&validated_path)
            .map_err(|e| shared_utils::handle_file_error(e, "read", &validated_path))?;

        // Perform replacement
        let replace_all = request.replace_all.unwrap_or(false);
        let new_content = if replace_all {
            content.replace(&request.old_string, &request.new_string)
        } else {
            // Replace only first occurrence
            if let Some(pos) = content.find(&request.old_string) {
                let mut result = String::with_capacity(content.len());
                result.push_str(&content[..pos]);
                result.push_str(&request.new_string);
                result.push_str(&content[pos + request.old_string.len()..]);
                result
            } else {
                return Err(rmcp::Error::invalid_request(
                    format!("String not found in file: '{}'", request.old_string),
                    None,
                ));
            }
        };

        // Ensure replacement occurred (for replace_all case)
        if !replace_all && new_content == content {
            return Err(rmcp::Error::invalid_request(
                format!("String not found in file: '{}'", request.old_string),
                None,
            ));
        }

        // Write updated content back to file
        std::fs::write(&validated_path, &new_content)
            .map_err(|e| shared_utils::handle_file_error(e, "write", &validated_path))?;

        let replacements = if replace_all {
            let count = content.matches(&request.old_string).count();
            format!(
                "Made {} replacements in file: {}",
                count,
                validated_path.display()
            )
        } else {
            format!("Made 1 replacement in file: {}", validated_path.display())
        };

        Ok(BaseToolImpl::create_success_response(replacements))
    }
}
