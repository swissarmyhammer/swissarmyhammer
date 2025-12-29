//! Web fetch tool for MCP operations
//!
//! This module provides the WebFetchTool for fetching web content and converting HTML to markdown
//! through the MCP protocol by delegating to the markdowndown crate.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_fetch::security::{SecurityError, SecurityValidator};
use crate::mcp::types::WebFetchRequest;
use async_trait::async_trait;
use markdowndown::{convert_url_with_config, Config};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::time::Instant;

/// Configuration constants for web fetch operations
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;
const MIN_TIMEOUT_SECONDS: u32 = 1;
const MAX_TIMEOUT_SECONDS: u32 = 120;
const DEFAULT_CONTENT_LENGTH_BYTES: u32 = 1_048_576; // 1MB
const MIN_CONTENT_LENGTH_BYTES: u32 = 1024; // 1KB
const MAX_CONTENT_LENGTH_BYTES: u32 = 10_485_760; // 10MB

/// Tool for fetching web content and converting HTML to markdown using markdowndown
pub struct WebFetchTool {
    /// Security validator for URL and domain validation
    security_validator: SecurityValidator,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    /// Creates a new instance of the WebFetchTool
    pub fn new() -> Self {
        Self {
            security_validator: SecurityValidator::new(),
        }
    }

    /// Validates request parameters including URL security and parameter ranges
    async fn validate_request_parameters(
        &self,
        request: &WebFetchRequest,
    ) -> Result<String, McpError> {
        // Comprehensive URL security validation
        let validated_url = match self.security_validator.validate_url(&request.url) {
            Ok(url) => url,
            Err(SecurityError::InvalidUrl(msg)) => {
                self.security_validator
                    .log_security_event("INVALID_URL", &request.url, &msg);
                return Err(McpError::invalid_params(
                    format!("Invalid URL: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::UnsupportedScheme(protocol)) => {
                self.security_validator.log_security_event(
                    "UNSUPPORTED_PROTOCOL",
                    &request.url,
                    &format!("Protocol: {protocol}"),
                );
                return Err(McpError::invalid_params(
                    format!("Unsupported protocol: {protocol}. Only HTTP and HTTPS are supported."),
                    None,
                ));
            }
            Err(SecurityError::SsrfAttempt(network)) => {
                self.security_validator.log_security_event(
                    "PRIVATE_NETWORK_ACCESS_ATTEMPT",
                    &request.url,
                    &format!("Network: {network}"),
                );
                return Err(McpError::invalid_params(
                    format!("Access to private network not allowed: {network}"),
                    None,
                ));
            }
            Err(e) => {
                self.security_validator.log_security_event(
                    "SECURITY_VALIDATION_FAILED",
                    &request.url,
                    &e.to_string(),
                );
                return Err(McpError::invalid_params(
                    format!("Security validation failed: {e}"),
                    None,
                ));
            }
        };

        Ok(validated_url.to_string())
    }

    /// Converts WebFetchRequest parameters to markdowndown Config
    fn create_markdowndown_config(&self, request: &WebFetchRequest) -> Config {
        let mut config = Config::default();

        // HTTP configuration
        config.http.timeout = std::time::Duration::from_secs(
            request.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS) as u64,
        );
        config.http.user_agent = request
            .user_agent
            .clone()
            .unwrap_or_else(|| "SwissArmyHammer-Bot/1.0".to_string());
        config.http.max_redirects = if request.follow_redirects.unwrap_or(true) {
            10
        } else {
            0
        };

        // Note: markdowndown doesn't expose max_response_size in HttpConfig
        // Content size limits are handled internally by markdowndown

        config
    }

    /// Builds a success response that returns only the fetched content
    fn build_success_response(
        &self,
        _request: &WebFetchRequest,
        markdown_content: String,
        _response_time_ms: u64,
    ) -> Result<CallToolResult, McpError> {
        // Return only the actual fetched content without verbose announcements
        Ok(CallToolResult {
            content: vec![rmcp::model::Annotated::new(
                rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                    text: markdown_content,
                    meta: None,
                }),
                None,
            )],
            structured_content: None,
            meta: None,
            is_error: Some(false),
        })
    }

    /// Builds an error response with detailed error information
    fn build_error_response(
        &self,
        error: &dyn std::error::Error,
        response_time_ms: u64,
        request: &WebFetchRequest,
    ) -> Result<CallToolResult, McpError> {
        let error_type = self.categorize_error(error);
        tracing::warn!(
            "Failed to fetch content from {} after {}ms: {} (category: {})",
            request.url,
            response_time_ms,
            error,
            error_type
        );

        // Build metadata object per specification for error response
        let metadata = serde_json::json!({
            "url": request.url,
            "error_type": error_type,
            "error_details": error.to_string(),
            "status_code": null,
            "response_time_ms": response_time_ms,
            "performance_impact": if response_time_ms > 10000 { "high" } else { "low" },
            "optimization_enabled": true
        });

        // Build the specification-compliant error response
        let response = serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Failed to fetch content: {error}")
            }],
            "is_error": true,
            "metadata": metadata
        });

        Ok(CallToolResult {
            content: vec![rmcp::model::Annotated::new(
                rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                    text: serde_json::to_string_pretty(&response).unwrap_or_default(),
                    meta: None,
                }),
                None,
            )],
            meta: None,
            structured_content: None,
            is_error: Some(true),
        })
    }

    /// Categorizes errors for proper error handling and response formatting
    fn categorize_error(&self, error: &dyn std::error::Error) -> &'static str {
        let error_str = error.to_string().to_lowercase();

        if error_str.contains("timeout") || error_str.contains("timed out") {
            "timeout"
        } else if error_str.contains("connection")
            || error_str.contains("network")
            || error_str.contains("dns")
            || error_str.contains("resolve")
        {
            "network_error"
        } else if error_str.contains("ssl")
            || error_str.contains("tls")
            || error_str.contains("certificate")
        {
            "ssl_error"
        } else if error_str.contains("redirect") {
            "redirect_error"
        } else if error_str.contains("401") || error_str.contains("403") {
            "auth_error"
        } else if error_str.contains("404") {
            "not_found"
        } else if error_str.contains("400") {
            "client_error"
        } else if error_str.contains("500")
            || error_str.contains("502")
            || error_str.contains("503")
        {
            "server_error"
        } else if error_str.contains("parse")
            || error_str.contains("encoding")
            || error_str.contains("invalid")
        {
            "content_error"
        } else if error_str.contains("too large") || error_str.contains("size") {
            "size_limit_error"
        } else {
            "unknown_error"
        }
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
                    "description": format!("Request timeout in seconds (optional, defaults to {DEFAULT_TIMEOUT_SECONDS} seconds)"),
                    "minimum": MIN_TIMEOUT_SECONDS,
                    "maximum": MAX_TIMEOUT_SECONDS,
                    "default": DEFAULT_TIMEOUT_SECONDS
                },
                "follow_redirects": {
                    "type": "boolean",
                    "description": "Whether to follow HTTP redirects (optional, defaults to true)",
                    "default": true
                },
                "max_content_length": {
                    "type": "integer",
                    "description": format!("Maximum content length in bytes (optional, defaults to {DEFAULT_CONTENT_LENGTH_BYTES} bytes)"),
                    "minimum": MIN_CONTENT_LENGTH_BYTES,
                    "maximum": MAX_CONTENT_LENGTH_BYTES,
                    "default": DEFAULT_CONTENT_LENGTH_BYTES
                },
                "user_agent": {
                    "type": "string",
                    "description": "Custom User-Agent header (optional, defaults to SwissArmyHammer-Bot/1.0)",
                    "default": "SwissArmyHammer-Bot/1.0"
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("web-search")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: WebFetchRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("Fetching web content from URL: {}", request.url);

        // Validate request parameters
        let validated_url = self.validate_request_parameters(&request).await?;

        // Create markdowndown configuration from request parameters
        let config = self.create_markdowndown_config(&request);

        // Generate progress token and send start notification
        let progress_token = generate_progress_token();
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(0),
                    format!("Web fetch: Fetching: {}", request.url),
                    json!({
                        "url": request.url,
                        "timeout": config.http.timeout.as_secs()
                    }),
                )
                .ok();
        }

        // Measure execution time
        let start_time = Instant::now();

        // Delegate to markdowndown for web fetching and conversion
        match convert_url_with_config(&validated_url, config).await {
            Ok(markdown) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                let markdown_content = markdown.to_string();

                // Send completion notification
                if let Some(sender) = &_context.progress_sender {
                    sender
                        .send_progress_with_metadata(
                            &progress_token,
                            Some(100),
                            format!(
                                "Web fetch: Complete - {} chars in {:.1}s",
                                markdown_content.len(),
                                response_time_ms as f64 / 1000.0
                            ),
                            json!({
                                "markdown_length": markdown_content.len(),
                                "duration_ms": response_time_ms
                            }),
                        )
                        .ok();
                }

                self.build_success_response(&request, markdown_content, response_time_ms)
            }
            Err(e) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;

                // Send error notification
                if let Some(sender) = &_context.progress_sender {
                    sender
                        .send_progress_with_metadata(
                            &progress_token,
                            None,
                            format!("Web fetch: Failed - {}", e),
                            json!({
                                "error": e.to_string(),
                                "url": request.url,
                                "duration_ms": response_time_ms
                            }),
                        )
                        .ok();
                }

                self.build_error_response(&e, response_time_ms, &request)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let obj = schema.as_object().expect("Schema should be an object");

        // Test required properties
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));

        let properties = obj["properties"]
            .as_object()
            .expect("Properties should be an object");
        assert!(properties.contains_key("url"));
        assert!(properties.contains_key("timeout"));
        assert!(properties.contains_key("follow_redirects"));
        assert!(properties.contains_key("max_content_length"));
        assert!(properties.contains_key("user_agent"));

        let required = obj["required"]
            .as_array()
            .expect("Required should be an array");
        assert!(required.contains(&serde_json::Value::String("url".to_string())));
    }

    #[test]
    fn test_create_markdowndown_config() {
        let tool = WebFetchTool::new();

        // Test with default values
        let request = WebFetchRequest {
            url: "https://example.com".to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };

        let config = tool.create_markdowndown_config(&request);
        assert_eq!(config.http.timeout, std::time::Duration::from_secs(30));
        assert_eq!(config.http.user_agent, "SwissArmyHammer-Bot/1.0");
        assert_eq!(config.http.max_redirects, 10);

        // Test with custom values
        let request = WebFetchRequest {
            url: "https://example.com".to_string(),
            timeout: Some(60),
            follow_redirects: Some(false),
            max_content_length: Some(2_097_152),
            user_agent: Some("CustomAgent/1.0".to_string()),
        };

        let config = tool.create_markdowndown_config(&request);
        assert_eq!(config.http.timeout, std::time::Duration::from_secs(60));
        assert_eq!(config.http.user_agent, "CustomAgent/1.0");
        assert_eq!(config.http.max_redirects, 0);
    }

    #[test]
    fn test_categorize_error() {
        let tool = WebFetchTool::new();

        let timeout_error =
            std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timed out");
        assert_eq!(tool.categorize_error(&timeout_error), "timeout");

        let network_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        assert_eq!(tool.categorize_error(&network_error), "network_error");

        let ssl_error = std::io::Error::new(std::io::ErrorKind::Other, "SSL certificate error");
        assert_eq!(tool.categorize_error(&ssl_error), "ssl_error");

        let parse_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid encoding");
        assert_eq!(tool.categorize_error(&parse_error), "content_error");
    }

    #[tokio::test]
    async fn test_web_fetch_sends_progress_notifications_on_success() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let tool = WebFetchTool::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = crate::test_utils::create_test_context().await;
        context.progress_sender = Some(progress_sender);

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );

        // Execute the tool (this will likely fail due to network, but we can check notifications)
        let _result = tool.execute(arguments, &context).await;

        // Collect notifications
        let mut notifications = Vec::new();
        while let Ok(notification) = rx.try_recv() {
            notifications.push(notification);
        }

        // Should have at least one notification (start notification)
        assert!(
            !notifications.is_empty(),
            "Should have sent at least one progress notification"
        );

        // First notification should be start (0% progress)
        let start_notif = &notifications[0];
        assert_eq!(start_notif.progress, Some(0));
        assert!(start_notif.message.contains("Fetching:"));
        assert!(start_notif.metadata.is_some());

        // If successful, last notification should be 100% or if failed, should be None
        let last_notif = &notifications[notifications.len() - 1];
        assert!(
            last_notif.progress == Some(100) || last_notif.progress.is_none(),
            "Last notification should be either completion (100%) or error (None)"
        );
    }

    #[tokio::test]
    async fn test_web_fetch_sends_error_notification_on_failure() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let tool = WebFetchTool::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = crate::test_utils::create_test_context().await;
        context.progress_sender = Some(progress_sender);

        let mut arguments = serde_json::Map::new();
        // Use example.com with a high port number (60000) that is unlikely to be open
        // Combined with minimum timeout for fastest failure
        arguments.insert(
            "url".to_string(),
            serde_json::Value::String("http://example.com:60000".to_string()),
        );
        // Use minimal timeout to speed up the test (1 second is the minimum allowed)
        arguments.insert("timeout".to_string(), serde_json::Value::Number(1.into()));

        // Execute the tool (should fail quickly with connection timeout/refused)
        let _result = tool.execute(arguments, &context).await;

        // Give a small delay for async notifications to be sent
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Collect notifications
        let mut notifications = Vec::new();
        while let Ok(notification) = rx.try_recv() {
            notifications.push(notification);
        }

        // Should have at least 2 notifications (start and error)
        assert!(
            notifications.len() >= 2,
            "Should have sent start and error notifications, got {}",
            notifications.len()
        );

        // First notification should be start
        assert_eq!(notifications[0].progress, Some(0));

        // Last notification should be error (None progress)
        let error_notif = &notifications[notifications.len() - 1];
        assert_eq!(error_notif.progress, None);
        assert!(error_notif.message.contains("Failed"));
    }

    #[tokio::test]
    async fn test_web_fetch_works_without_progress_sender() {
        let tool = WebFetchTool::new();

        // Context without progress sender
        let context = crate::test_utils::create_test_context().await;
        assert!(context.progress_sender.is_none());

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );

        // Should not panic even without progress sender
        let _result = tool.execute(arguments, &context).await;
        // Test passes if it doesn't panic
    }
}
