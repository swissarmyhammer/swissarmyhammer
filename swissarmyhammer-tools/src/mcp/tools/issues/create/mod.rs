//! Issue creation tool for MCP operations
//!
//! This module provides the CreateIssueTool for creating new issues through the MCP protocol.

use crate::mcp::responses::create_issue_response;
use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::CreateIssueRequest;
use crate::mcp::utils::validate_issue_name;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

/// Tool for creating new issues
#[derive(Default)]
pub struct CreateIssueTool;

impl CreateIssueTool {
    /// Creates a new instance of the CreateIssueTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for CreateIssueTool {
    fn name(&self) -> &'static str {
        "issue_create"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("issues", "create")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": ["string", "null"],
                    "description": "Name of the issue (optional for nameless issues)"
                },
                "content": {
                    "type": "string",
                    "description": "Markdown content of the issue"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: CreateIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for issue creation
        context
            .rate_limiter
            .check_rate_limit("unknown", "issue_create", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for issue creation: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Creating issue: {:?}", request.name);

        // Validate issue name using shared validation logic, or use empty string for nameless issues
        let validated_name = match &request.name {
            Some(name) => {
                McpValidation::validate_not_empty(name.as_str(), "issue name")
                    .map_err(|e| McpErrorHandler::handle_error(e, "validate issue name"))?;
                validate_issue_name(name.as_str())?
            }
            None => String::new(), // Empty name for nameless issues - skip validation
        };

        // Allow empty content for issues - issues can be created as placeholders and filled in later

        let issue_storage = context.issue_storage.write().await;
        match issue_storage
            .create_issue(validated_name, request.content)
            .await
        {
            Ok(issue) => {
                // Get the full issue info for the response (includes file path)
                match issue_storage.get_issue_info(&issue.name).await {
                    Ok(issue_info) => {
                        tracing::info!(
                            "Created issue {} from current branch",
                            issue_info.issue.name
                        );
                        Ok(create_issue_response(&issue_info))
                    }
                    Err(e) => Err(McpErrorHandler::handle_error(
                        swissarmyhammer::error::CommonError::Other { message: e.to_string() },
                        "get created issue info",
                    )),
                }
            }
            Err(e) => Err(McpErrorHandler::handle_error(
                swissarmyhammer::error::CommonError::Other { message: e.to_string() },
                "create issue",
            )),
        }
    }
}
