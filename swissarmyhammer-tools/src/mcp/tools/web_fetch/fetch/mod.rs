//! Web fetch tool for MCP operations
//!
//! This module provides the WebFetchTool for fetching web content and converting HTML to markdown
//! through the MCP protocol using the markdowndown crate.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::WebFetchRequest;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Tool for fetching web content and converting HTML to markdown
#[derive(Default)]
pub struct WebFetchTool;

impl WebFetchTool {
    /// Creates a new instance of the WebFetchTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("web_fetch", "fetch")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The URL to fetch content from (must be a valid HTTP/HTTPS URL)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Request timeout in seconds (optional, defaults to 30 seconds)",
                    "minimum": 5,
                    "maximum": 120,
                    "default": 30
                },
                "follow_redirects": {
                    "type": "boolean",
                    "description": "Whether to follow HTTP redirects (optional, defaults to true)",
                    "default": true
                },
                "max_content_length": {
                    "type": "integer",
                    "description": "Maximum content length in bytes (optional, defaults to 1MB)",
                    "minimum": 1024,
                    "maximum": 10485760,
                    "default": 1048576
                },
                "user_agent": {
                    "type": "string",
                    "description": "Custom User-Agent header (optional, defaults to SwissArmyHammer-Bot/1.0)",
                    "default": "SwissArmyHammer-Bot/1.0"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: WebFetchRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for web fetch operations
        context
            .rate_limiter
            .check_rate_limit("unknown", "web_fetch", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for web fetch: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Fetching web content from URL: {}", request.url);

        // Validate URL is not empty and has valid scheme
        McpValidation::validate_not_empty(&request.url, "URL")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate URL"))?;

        // Validate URL scheme (HTTP/HTTPS only)
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
            return Err(McpError::invalid_params(
                "URL must use HTTP or HTTPS scheme".to_string(),
                None,
            ));
        }

        // Validate optional timeout range
        if let Some(timeout) = request.timeout {
            if timeout < 5 || timeout > 120 {
                return Err(McpError::invalid_params(
                    "Timeout must be between 5 and 120 seconds".to_string(),
                    None,
                ));
            }
        }

        // Validate optional max_content_length range
        if let Some(max_length) = request.max_content_length {
            if max_length < 1024 || max_length > 10_485_760 {
                return Err(McpError::invalid_params(
                    "Maximum content length must be between 1KB and 10MB".to_string(),
                    None,
                ));
            }
        }

        // TODO: Implement actual web fetching using markdowndown crate
        // This is a placeholder implementation that will be completed in subsequent issues
        
        tracing::info!("Web fetch requested for: {}", request.url);
        
        // Create placeholder response indicating the tool structure is ready
        let response_content = format!(
            "WebFetch tool structure created successfully.\nURL: {}\nTimeout: {:?}s\nFollow redirects: {:?}\nMax content length: {:?} bytes\nUser agent: {:?}",
            request.url,
            request.timeout.unwrap_or(30),
            request.follow_redirects.unwrap_or(true),
            request.max_content_length.unwrap_or(1_048_576),
            request.user_agent.as_deref().unwrap_or("SwissArmyHammer-Bot/1.0")
        );

        Ok(BaseToolImpl::create_success_response(&response_content))
    }
}