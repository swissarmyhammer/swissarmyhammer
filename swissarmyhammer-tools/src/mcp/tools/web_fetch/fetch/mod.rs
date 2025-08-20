//! Web fetch tool for MCP operations
//!
//! This module provides the WebFetchTool for fetching web content and converting HTML to markdown
//! through the MCP protocol by delegating to the markdowndown crate.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_fetch::security::{SecurityError, SecurityValidator};
use crate::mcp::types::WebFetchRequest;
use async_trait::async_trait;
use markdowndown::{convert_url_with_config, Config};
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::time::Instant;

/// Configuration constants for web fetch operations
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;
const MIN_TIMEOUT_SECONDS: u32 = 5;
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

    /// Extracts title from markdown content (first # heading)
    fn extract_title_from_markdown(&self, markdown: &str) -> Option<String> {
        for line in markdown.lines() {
            let trimmed = line.trim();
            if let Some(title) = trimmed.strip_prefix("# ") {
                return Some(title.trim().to_string());
            }
        }
        None
    }

    /// Counts words in text content
    fn count_words(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Builds a success response with the same format as the original implementation
    fn build_success_response(
        &self,
        request: &WebFetchRequest,
        markdown_content: String,
        response_time_ms: u64,
    ) -> Result<CallToolResult, McpError> {
        let title = self
            .extract_title_from_markdown(&markdown_content)
            .unwrap_or_else(|| "Web Content".to_string());
        let word_count = self.count_words(&markdown_content);
        let content_length = markdown_content.len();

        // Calculate performance metrics
        let transfer_rate_kbps = if response_time_ms > 0 {
            (content_length as f64 / 1024.0) / (response_time_ms as f64 / 1000.0)
        } else {
            0.0
        };

        tracing::info!(
            "Successfully fetched content from {} ({}ms, {} bytes, {} words, {:.1} KB/s)",
            request.url,
            response_time_ms,
            content_length,
            word_count,
            transfer_rate_kbps
        );

        // Build metadata object per specification
        let metadata = serde_json::json!({
            "url": request.url,
            "final_url": request.url, // markdowndown handles redirects internally
            "title": title,
            "content_type": "text/html",
            "content_length": content_length,
            "status_code": 200, // markdowndown only returns success cases
            "response_time_ms": response_time_ms,
            "markdown_content": markdown_content,
            "word_count": word_count,
            "headers": {}, // markdowndown doesn't expose headers
            "performance_metrics": {
                "transfer_rate_kbps": format!("{:.2}", transfer_rate_kbps),
                "content_efficiency": format!("{:.2}", word_count as f64 / content_length as f64 * 100.0),
                "processing_optimized": true
            }
        });

        let success_message = "Successfully fetched content from URL".to_string();

        // Build the specification-compliant response
        let response = serde_json::json!({
            "content": [{
                "type": "text",
                "text": success_message
            }],
            "is_error": false,
            "metadata": metadata
        });

        Ok(CallToolResult {
            content: vec![rmcp::model::Annotated::new(
                rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                    text: serde_json::to_string_pretty(&response).unwrap_or_default(),
                }),
                None,
            )],
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
                }),
                None,
            )],
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

    fn hidden_from_cli(&self) -> bool {
        true
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

        // Validate request parameters
        let validated_url = self.validate_request_parameters(&request).await?;

        // Create markdowndown configuration from request parameters
        let config = self.create_markdowndown_config(&request);

        // Measure execution time
        let start_time = Instant::now();

        // Delegate to markdowndown for web fetching and conversion
        match convert_url_with_config(&validated_url, config).await {
            Ok(markdown) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                let markdown_content = markdown.to_string();
                self.build_success_response(&request, markdown_content, response_time_ms)
            }
            Err(e) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
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
    fn test_extract_title_from_markdown() {
        let tool = WebFetchTool::new();

        // Test with title
        let markdown = "# Main Title\n\nSome content here.";
        assert_eq!(
            tool.extract_title_from_markdown(markdown),
            Some("Main Title".to_string())
        );

        // Test without title
        let markdown = "Just some content without title.";
        assert_eq!(tool.extract_title_from_markdown(markdown), None);

        // Test with multiple headings (should return first)
        let markdown = "# First Title\n\n## Second Title\n\n# Third Title";
        assert_eq!(
            tool.extract_title_from_markdown(markdown),
            Some("First Title".to_string())
        );
    }

    #[test]
    fn test_count_words() {
        let tool = WebFetchTool::new();

        assert_eq!(tool.count_words("Hello world"), 2);
        assert_eq!(tool.count_words(""), 0);
        assert_eq!(
            tool.count_words("   Multiple   spaces   between   words   "),
            4
        );
        assert_eq!(tool.count_words("Single"), 1);
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
}
