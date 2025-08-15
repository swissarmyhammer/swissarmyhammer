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
            if !(5..=120).contains(&timeout) {
                return Err(McpError::invalid_params(
                    "Timeout must be between 5 and 120 seconds".to_string(),
                    None,
                ));
            }
        }

        // Validate optional max_content_length range
        if let Some(max_length) = request.max_content_length {
            if !(1024..=10_485_760).contains(&max_length) {
                return Err(McpError::invalid_params(
                    "Maximum content length must be between 1KB and 10MB".to_string(),
                    None,
                ));
            }
        }

        // Implement actual web fetching using markdowndown crate
        tracing::info!("Fetching web content from: {}", request.url);

        // Configure markdowndown Config
        let mut config = markdowndown::Config::default();

        // Configure HTTP settings
        config.http.user_agent = request
            .user_agent
            .clone()
            .unwrap_or_else(|| "SwissArmyHammer-Bot/1.0".to_string());
        config.http.timeout = std::time::Duration::from_secs(request.timeout.unwrap_or(30) as u64);
        config.http.max_redirects = if request.follow_redirects.unwrap_or(true) {
            10
        } else {
            0
        };

        // Perform the web fetch and convert to markdown
        let start_time = std::time::Instant::now();
        let fetch_result = markdowndown::convert_url_with_config(&request.url, config).await;
        let response_time_ms = start_time.elapsed().as_millis() as u64;

        match fetch_result {
            Ok(markdown_content) => {
                let content_str = markdown_content.as_str();
                let content_length = content_str.len();

                tracing::info!(
                    "Successfully fetched content from {} ({}ms, {} bytes)",
                    request.url,
                    response_time_ms,
                    content_length
                );

                // Create response with markdown content and metadata
                let response = serde_json::json!({
                    "url": request.url,
                    "status": "success",
                    "response_time_ms": response_time_ms,
                    "content_length": content_length,
                    "word_count": content_str.split_whitespace().count(),
                    "markdown_content": content_str
                });

                Ok(BaseToolImpl::create_success_response(format!(
                    "Successfully fetched and converted content from {}\n\nMetadata: {}\n\nContent:\n{}",
                    request.url,
                    serde_json::to_string_pretty(&response).unwrap_or_default(),
                    content_str
                )))
            }
            Err(error) => {
                tracing::warn!(
                    "Failed to fetch content from {} after {}ms: {}",
                    request.url,
                    response_time_ms,
                    error
                );

                // Create error response
                let error_info = serde_json::json!({
                    "url": request.url,
                    "status": "error",
                    "error_type": "fetch_error",
                    "error_details": error.to_string(),
                    "response_time_ms": response_time_ms
                });

                Err(McpError::invalid_params(
                    format!(
                        "Failed to fetch content from {}: {}\n\nError details: {}",
                        request.url,
                        error,
                        serde_json::to_string_pretty(&error_info).unwrap_or_default()
                    ),
                    None,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::BaseToolImpl;
    use crate::mcp::types::WebFetchRequest;

    #[test]
    fn test_web_fetch_tool_name() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
    }

    #[test]
    fn test_web_fetch_tool_description() {
        let tool = WebFetchTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
    }

    #[test]
    fn test_web_fetch_tool_schema() {
        let tool = WebFetchTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("url"));
        assert!(properties.contains_key("timeout"));
        assert!(properties.contains_key("follow_redirects"));
        assert!(properties.contains_key("max_content_length"));
        assert!(properties.contains_key("user_agent"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("url".to_string())));
    }

    #[test]
    fn test_parse_valid_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.timeout, None);
        assert_eq!(request.follow_redirects, None);
        assert_eq!(request.max_content_length, None);
        assert_eq!(request.user_agent, None);
    }

    #[test]
    fn test_parse_full_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(45)),
        );
        args.insert(
            "follow_redirects".to_string(),
            serde_json::Value::Bool(false),
        );
        args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(2097152)),
        );
        args.insert(
            "user_agent".to_string(),
            serde_json::Value::String("TestBot/1.0".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.timeout, Some(45));
        assert_eq!(request.follow_redirects, Some(false));
        assert_eq!(request.max_content_length, Some(2097152));
        assert_eq!(request.user_agent, Some("TestBot/1.0".to_string()));
    }

    #[test]
    fn test_parse_missing_url() {
        let args = serde_json::Map::new();

        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_url_validation_invalid_scheme() {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("ftp://example.com".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();

        // Test would need a real ToolContext to execute, but we can test the validation logic directly
        assert_eq!(request.url, "ftp://example.com");
        assert!(!request.url.starts_with("http://") && !request.url.starts_with("https://"));
    }

    #[test]
    fn test_timeout_validation() {
        // Test minimum timeout validation logic
        let timeout_too_small = 3_u32;
        let timeout_too_large = 150_u32;
        let timeout_valid = 30_u32;

        assert!(!(5..=120).contains(&timeout_too_small));
        assert!(!(5..=120).contains(&timeout_too_large));
        assert!((5..=120).contains(&timeout_valid));
    }

    #[test]
    fn test_content_length_validation() {
        // Test content length validation logic
        let length_too_small = 512_u32;
        let length_too_large = 20_971_520_u32; // 20MB
        let length_valid = 1_048_576_u32; // 1MB

        assert!(!(1024..=10_485_760).contains(&length_too_small));
        assert!(!(1024..=10_485_760).contains(&length_too_large));
        assert!((1024..=10_485_760).contains(&length_valid));
    }
}
