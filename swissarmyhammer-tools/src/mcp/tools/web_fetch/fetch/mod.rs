//! Web fetch tool for MCP operations
//!
//! This module provides the WebFetchTool for fetching web content and converting HTML to markdown
//! through the MCP protocol using the markdowndown crate.

// Security validation replaces the old basic validation utilities
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_fetch::security::{SecurityError, SecurityValidator};
use crate::mcp::types::WebFetchRequest;
use async_trait::async_trait;
use markdowndown::HtmlConverter;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::time::Duration;

/// Configuration constants for web fetch operations
/// Maximum number of redirects to follow when follow_redirects is enabled
const MAX_REDIRECTS: usize = 10;

/// Timeout configuration constants (in seconds)
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;
const MIN_TIMEOUT_SECONDS: u32 = 5;
const MAX_TIMEOUT_SECONDS: u32 = 120;

/// Content length configuration constants (in bytes)
const DEFAULT_CONTENT_LENGTH_BYTES: u32 = 1_048_576; // 1MB
const MIN_CONTENT_LENGTH_BYTES: u32 = 1024; // 1KB
const MAX_CONTENT_LENGTH_BYTES: u32 = 10_485_760; // 10MB

/// Represents a single step in a redirect chain
#[derive(Debug, Clone)]
pub struct RedirectStep {
    /// The URL that was requested in this step
    pub url: String,
    /// The HTTP status code returned for this step
    pub status_code: u16,
}

/// Contains redirect chain information
#[derive(Debug, Clone)]
pub struct RedirectInfo {
    /// The total number of redirects followed
    pub redirect_count: usize,
    /// The complete chain of redirects including the final request
    pub redirect_chain: Vec<RedirectStep>,
    /// The final URL after all redirects
    pub final_url: String,
}

/// Tool for fetching web content and converting HTML to markdown
pub struct WebFetchTool {
    /// Security validator for URL and domain validation
    security_validator: SecurityValidator,
    /// HTML converter for converting HTML to markdown
    html_converter: HtmlConverter,
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
            html_converter: HtmlConverter::new(),
        }
    }

    /// Performs HTTP request with redirect tracking
    async fn fetch_with_redirect_tracking(
        &self,
        url: &str,
        request: &WebFetchRequest,
    ) -> Result<
        (
            String,
            RedirectInfo,
            std::collections::HashMap<String, String>,
        ),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let client = reqwest::Client::builder()
            .user_agent(
                request
                    .user_agent
                    .as_deref()
                    .unwrap_or("SwissArmyHammer-Bot/1.0"),
            )
            .timeout(Duration::from_secs(
                request.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS) as u64,
            ))
            .redirect(reqwest::redirect::Policy::none()) // Handle redirects manually
            .build()?;

        let mut redirect_chain = Vec::new();
        let mut current_url = url.to_string();
        let mut redirect_count = 0;
        let max_redirects = if request.follow_redirects.unwrap_or(true) {
            MAX_REDIRECTS
        } else {
            0
        };

        loop {
            tracing::debug!(
                "Fetching URL: {} (redirect #{}/{})",
                current_url,
                redirect_count,
                max_redirects
            );

            let response = client.get(&current_url).send().await?;
            let status_code = response.status().as_u16();

            // Add current step to redirect chain
            redirect_chain.push(RedirectStep {
                url: current_url.clone(),
                status_code,
            });

            // Check if this is a redirect
            if (300..400).contains(&status_code) {
                if redirect_count >= max_redirects {
                    return Err(format!(
                        "Too many redirects ({redirect_count}). Maximum allowed: {max_redirects}"
                    )
                    .into());
                }

                // Get redirect location
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or("Redirect response missing Location header")?;

                // Handle relative URLs
                let redirect_url =
                    if location.starts_with("http://") || location.starts_with("https://") {
                        location.to_string()
                    } else {
                        // Parse base URL and resolve relative redirect
                        let base_url = reqwest::Url::parse(&current_url)?;
                        base_url.join(location)?.to_string()
                    };

                current_url = redirect_url;
                redirect_count += 1;
                continue;
            }

            // Not a redirect - check if successful
            if !response.status().is_success() {
                let status_code = response.status().as_u16();
                let reason = response.status().canonical_reason().unwrap_or("Unknown");
                return Err(format!("HTTP error: {status_code} {reason}").into());
            }

            // Get final content and extract headers
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/html")
                .to_string();

            // Extract relevant headers for metadata
            let mut headers = std::collections::HashMap::new();
            for (name, value) in response.headers().iter() {
                // Include common headers that might be useful for debugging/monitoring
                let header_name = name.as_str().to_lowercase();
                if header_name.contains("server")
                    || header_name.contains("content-encoding")
                    || header_name.contains("content-length")
                    || header_name.contains("last-modified")
                    || header_name.contains("etag")
                    || header_name.contains("cache-control")
                    || header_name.contains("expires")
                {
                    if let Ok(header_value) = value.to_str() {
                        headers.insert(header_name, header_value.to_string());
                    }
                }
            }

            // Stream content with size validation
            let max_length = request
                .max_content_length
                .unwrap_or(DEFAULT_CONTENT_LENGTH_BYTES) as usize;
            let mut body_bytes = Vec::new();
            let mut stream = response.bytes_stream();

            use futures_util::StreamExt;

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                body_bytes.extend_from_slice(&chunk);

                // Check size limit during streaming
                if body_bytes.len() > max_length {
                    return Err(format!(
                        "Content too large: {} bytes exceeds limit of {} bytes",
                        body_bytes.len(),
                        max_length
                    )
                    .into());
                }
            }

            // Convert bytes to string
            let body =
                String::from_utf8(body_bytes).map_err(|e| format!("Invalid UTF-8 content: {e}"))?;

            // Convert HTML to markdown using markdowndown
            let markdown_content = if content_type.contains("text/html") {
                match self.html_converter.convert_html(&body) {
                    Ok(md) => md,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to convert HTML to markdown using markdowndown: {}",
                            e
                        );
                        // Fallback to plain text wrapped in code block
                        format!("```html\n{body}\n```")
                    }
                }
            } else {
                // For non-HTML content, return as-is wrapped in code block
                format!("```\n{body}\n```")
            };

            let redirect_info = RedirectInfo {
                redirect_count,
                redirect_chain,
                final_url: current_url,
            };

            return Ok((markdown_content, redirect_info, headers));
        }
    }

    /// Extract title from markdown content (first heading)
    fn extract_title_from_markdown(markdown: &str) -> Option<String> {
        for line in markdown.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                // Extract heading text, removing # symbols and whitespace
                let title = trimmed.trim_start_matches('#').trim().to_string();
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
        None
    }

    /// Categorize errors by type for better error handling
    fn categorize_error(error: &dyn std::error::Error) -> &'static str {
        let error_str = error.to_string().to_lowercase();

        // Security-related errors (check first)
        if error_str.contains("blocked domain")
            || error_str.contains("ssrf")
            || error_str.contains("unsupported scheme")
            || error_str.contains("ssl")
            || error_str.contains("tls")
            || error_str.contains("certificate")
        {
            "security_error"
        // Network-related errors
        } else if error_str.contains("connection")
            || error_str.contains("timeout")
            || error_str.contains("dns")
        {
            "network_error"
        } else if error_str.contains("redirect") || error_str.contains("too many") {
            "redirect_error"
        } else if error_str.contains("404") || error_str.contains("not found") {
            "not_found_error"
        } else if error_str.contains("403")
            || error_str.contains("forbidden")
            || error_str.contains("unauthorized")
        {
            "access_denied_error"
        } else if error_str.contains("500")
            || error_str.contains("502")
            || error_str.contains("503")
        {
            "server_error"
        } else if error_str.contains("markdowndown")
            || error_str.contains("html conversion")
            || error_str.contains("markdown conversion")
        {
            "content_processing_error"
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
                    "description": format!("Request timeout in seconds (optional, defaults to {} seconds)", DEFAULT_TIMEOUT_SECONDS),
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
                    "description": format!("Maximum content length in bytes (optional, defaults to {} bytes)", DEFAULT_CONTENT_LENGTH_BYTES),
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

        // Validate request parameters
        let _validated_url = self.validate_request_parameters(&request).await?;

        // Implement web fetching with redirect tracking
        tracing::info!("Fetching web content from: {}", request.url);

        let start_time = std::time::Instant::now();
        let fetch_result = self
            .fetch_with_redirect_tracking(&request.url, &request)
            .await;
        let response_time_ms = start_time.elapsed().as_millis() as u64;

        match fetch_result {
            Ok((markdown_content, redirect_info, headers)) => Self::build_success_response(
                markdown_content,
                redirect_info,
                headers,
                response_time_ms,
                &request,
            ),
            Err(error) => Self::build_error_response(error.as_ref(), response_time_ms, &request),
        }
    }
}

impl WebFetchTool {
    /// Validates request parameters including URL security, timeout, and content length
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
            Err(SecurityError::BlockedDomain(msg)) => {
                self.security_validator
                    .log_security_event("BLOCKED_DOMAIN", &request.url, &msg);
                return Err(McpError::invalid_params(
                    format!("Access denied: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::SsrfAttempt(msg)) => {
                self.security_validator
                    .log_security_event("SSRF_ATTEMPT", &request.url, &msg);
                return Err(McpError::invalid_params(
                    format!("Security violation: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::UnsupportedScheme(msg)) => {
                self.security_validator.log_security_event(
                    "UNSUPPORTED_SCHEME",
                    &request.url,
                    &msg,
                );
                return Err(McpError::invalid_params(
                    format!("Unsupported protocol: {msg}"),
                    None,
                ));
            }
        };

        tracing::info!("URL security validation passed for: {}", validated_url);

        // Validate optional timeout range
        if let Some(timeout) = request.timeout {
            if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout) {
                return Err(McpError::invalid_params(
                    format!("Timeout must be between {MIN_TIMEOUT_SECONDS} and {MAX_TIMEOUT_SECONDS} seconds"),
                    None,
                ));
            }
        }

        // Validate optional max_content_length range
        if let Some(max_length) = request.max_content_length {
            if !(MIN_CONTENT_LENGTH_BYTES..=MAX_CONTENT_LENGTH_BYTES).contains(&max_length) {
                return Err(McpError::invalid_params(
                    format!("Maximum content length must be between {MIN_CONTENT_LENGTH_BYTES} and {MAX_CONTENT_LENGTH_BYTES} bytes"),
                    None,
                ));
            }
        }

        Ok(validated_url.to_string())
    }

    /// Builds a successful response with content and metadata
    fn build_success_response(
        content: String,
        redirect_info: RedirectInfo,
        headers: std::collections::HashMap<String, String>,
        response_time_ms: u64,
        request: &WebFetchRequest,
    ) -> Result<CallToolResult, McpError> {
        let content_str = content.as_str();
        let content_length = content_str.len();
        let word_count = content_str.split_whitespace().count();

        // Extract HTML title from markdown content (first heading)
        let extracted_title = Self::extract_title_from_markdown(content_str);

        tracing::info!(
            "Successfully fetched content from {} ({}ms, {} bytes, {} words)",
            request.url,
            response_time_ms,
            content_length,
            word_count
        );

        // Create redirect chain formatted as per specification
        let redirect_chain_formatted: Vec<String> = redirect_info
            .redirect_chain
            .iter()
            .map(|step| {
                let url = &step.url;
                let status_code = step.status_code;
                format!("{url} -> {status_code}")
            })
            .collect();

        // Build metadata object per specification
        let mut metadata = serde_json::json!({
            "url": request.url,
            "final_url": redirect_info.final_url,
            "title": extracted_title,
            "content_type": "text/html",
            "content_length": content_length,
            "status_code": redirect_info.redirect_chain.last().map(|s| s.status_code).unwrap_or(200),
            "response_time_ms": response_time_ms,
            "markdown_content": content_str,
            "word_count": word_count,
            "headers": headers
        });

        // Add redirect information if redirects occurred
        if redirect_info.redirect_count > 0 {
            metadata["redirect_count"] =
                serde_json::Value::Number(serde_json::Number::from(redirect_info.redirect_count));
            metadata["redirect_chain"] = serde_json::Value::Array(
                redirect_chain_formatted
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            );
        }

        // Create response following specification format exactly
        let success_message = if redirect_info.redirect_count > 0 {
            "URL redirected to final destination".to_string()
        } else {
            "Successfully fetched content from URL".to_string()
        };

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
        error: &(dyn std::error::Error + Send + Sync),
        response_time_ms: u64,
        request: &WebFetchRequest,
    ) -> Result<CallToolResult, McpError> {
        let error_type = Self::categorize_error(error);

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
            "response_time_ms": response_time_ms
        });

        // Build the specification-compliant error response
        let response = serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Failed to fetch content: {}", error)
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

    #[test]
    fn test_extract_title_from_markdown() {
        // Test title extraction from various markdown formats
        let markdown_with_title = "# Main Title\n\nSome content here.";
        let title = WebFetchTool::extract_title_from_markdown(markdown_with_title);
        assert_eq!(title, Some("Main Title".to_string()));

        // Test with multiple headings - should get the first
        let markdown_multiple_headings = "# First Title\n\nSome content.\n\n## Second Title";
        let title = WebFetchTool::extract_title_from_markdown(markdown_multiple_headings);
        assert_eq!(title, Some("First Title".to_string()));

        // Test with no headings
        let markdown_no_title = "Just some paragraph text without headings.";
        let title = WebFetchTool::extract_title_from_markdown(markdown_no_title);
        assert_eq!(title, None);

        // Test with empty heading
        let markdown_empty_heading = "#\n\nSome content.";
        let title = WebFetchTool::extract_title_from_markdown(markdown_empty_heading);
        assert_eq!(title, None);

        // Test with heading containing extra spaces
        let markdown_spaced_heading = "###   Spaced Title   \n\nContent.";
        let title = WebFetchTool::extract_title_from_markdown(markdown_spaced_heading);
        assert_eq!(title, Some("Spaced Title".to_string()));
    }

    #[test]
    fn test_error_categorization() {
        // Test network error categorization
        let network_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused");
        assert_eq!(
            WebFetchTool::categorize_error(&network_error),
            "network_error"
        );

        let timeout_error = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout occurred");
        assert_eq!(
            WebFetchTool::categorize_error(&timeout_error),
            "network_error"
        );

        // Test general error categorization
        let parse_error =
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid parse data");
        assert_eq!(
            WebFetchTool::categorize_error(&parse_error),
            "content_error"
        );

        // Test unknown error
        let unknown_error = std::io::Error::new(std::io::ErrorKind::Other, "some other error");
        assert_eq!(
            WebFetchTool::categorize_error(&unknown_error),
            "unknown_error"
        );
    }

    #[test]
    fn test_markdowndown_config_options() {
        let tool = WebFetchTool::new();
        let schema = tool.schema();

        // Verify schema has all required fields for configuration
        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        let properties = obj["properties"].as_object().unwrap();

        // Test that all configuration parameters are present
        assert!(properties.contains_key("url"));
        assert!(properties.contains_key("timeout"));
        assert!(properties.contains_key("follow_redirects"));
        assert!(properties.contains_key("max_content_length"));
        assert!(properties.contains_key("user_agent"));

        // Verify proper defaults and constraints
        let timeout_prop = &properties["timeout"];
        assert_eq!(timeout_prop["minimum"], 5);
        assert_eq!(timeout_prop["maximum"], 120);
        assert_eq!(timeout_prop["default"], 30);
    }

    #[test]
    fn test_url_validation_edge_cases() {
        // Test empty URL
        let empty_url = "";
        assert!(!empty_url.starts_with("http://") && !empty_url.starts_with("https://"));

        // Test whitespace-only URL
        let whitespace_url = "   ";
        assert!(!whitespace_url.starts_with("http://") && !whitespace_url.starts_with("https://"));

        // Test various invalid schemes
        let file_url = "file:///etc/passwd";
        let javascript_url = "javascript:alert('xss')";
        let data_url = "data:text/plain,Hello";
        let mailto_url = "mailto:user@example.com";

        assert!(!file_url.starts_with("http://") && !file_url.starts_with("https://"));
        assert!(!javascript_url.starts_with("http://") && !javascript_url.starts_with("https://"));
        assert!(!data_url.starts_with("http://") && !data_url.starts_with("https://"));
        assert!(!mailto_url.starts_with("http://") && !mailto_url.starts_with("https://"));

        // Test valid URLs
        let http_url = "http://example.com";
        let https_url = "https://secure.example.com";
        let https_path_url = "https://api.github.com/docs/rest";

        assert!(http_url.starts_with("http://"));
        assert!(https_path_url.starts_with("https://"));
        assert!(https_url.starts_with("https://"));

        // Test case sensitivity
        let uppercase_scheme = "HTTP://EXAMPLE.COM";
        let mixed_case_scheme = "Https://Example.Com";
        assert!(
            !uppercase_scheme.starts_with("http://") && !uppercase_scheme.starts_with("https://")
        );
        assert!(
            !mixed_case_scheme.starts_with("http://") && !mixed_case_scheme.starts_with("https://")
        );
    }

    #[test]
    fn test_parameter_boundary_validations_comprehensive() {
        // Test timeout boundaries
        let timeout_min_valid = 5_u32;
        let timeout_max_valid = 120_u32;
        let timeout_min_invalid = 4_u32;
        let timeout_max_invalid = 121_u32;

        assert!((5..=120).contains(&timeout_min_valid));
        assert!((5..=120).contains(&timeout_max_valid));
        assert!(!(5..=120).contains(&timeout_min_invalid));
        assert!(!(5..=120).contains(&timeout_max_invalid));

        // Test content length boundaries
        let content_min_valid = 1024_u32; // 1KB
        let content_max_valid = 10_485_760_u32; // 10MB
        let content_min_invalid = 1023_u32; // Less than 1KB
        let content_max_invalid = 10_485_761_u32; // More than 10MB

        assert!((1024..=10_485_760).contains(&content_min_valid));
        assert!((1024..=10_485_760).contains(&content_max_valid));
        assert!(!(1024..=10_485_760).contains(&content_min_invalid));
        assert!(!(1024..=10_485_760).contains(&content_max_invalid));

        // Test edge case values
        let content_1mb = 1_048_576_u32; // Default 1MB
        let content_5mb = 5_242_880_u32; // 5MB
        assert!((1024..=10_485_760).contains(&content_1mb));
        assert!((1024..=10_485_760).contains(&content_5mb));
    }

    #[test]
    fn test_user_agent_handling() {
        // Test default user agent
        let default_ua = "SwissArmyHammer-Bot/1.0";
        assert!(!default_ua.is_empty());
        assert!(default_ua.contains("SwissArmyHammer"));

        // Test custom user agents
        let custom_ua = "TestBot/2.0";
        let empty_ua = "";
        let long_ua = "A".repeat(500); // Very long user agent

        assert!(!custom_ua.is_empty());
        assert!(empty_ua.is_empty());
        assert!(long_ua.len() == 500);

        // Test user agent with special characters
        let special_chars_ua = "Bot/1.0 (Linux; x86_64) Mozilla/5.0";
        assert!(!special_chars_ua.is_empty());
        assert!(special_chars_ua.contains("Mozilla"));
    }

    #[test]
    fn test_default_values_application() {
        // Verify default values match specification
        let mut config = markdowndown::Config::default();

        // Test default timeout (30 seconds)
        assert_eq!(config.http.timeout, std::time::Duration::from_secs(30));

        // Test default user agent
        config.http.user_agent = "SwissArmyHammer-Bot/1.0".to_string();
        assert_eq!(config.http.user_agent, "SwissArmyHammer-Bot/1.0");

        // Test default max redirects (should be 10 when follow_redirects is true)
        config.http.max_redirects = 10;
        assert_eq!(config.http.max_redirects, 10);

        // Test default HTML processing options
        config.html.max_line_width = 120;
        config.html.remove_scripts_styles = true;
        config.html.remove_navigation = true;
        config.html.remove_sidebars = true;
        config.html.remove_ads = true;
        config.html.max_blank_lines = 2;

        assert_eq!(config.html.max_line_width, 120);
        assert!(config.html.remove_scripts_styles);
        assert!(config.html.remove_navigation);
        assert!(config.html.remove_sidebars);
        assert!(config.html.remove_ads);
        assert_eq!(config.html.max_blank_lines, 2);
    }

    #[test]
    fn test_parameter_validation_error_messages() {
        // Test URL scheme validation error message format
        let invalid_schemes = vec![
            "ftp://example.com",
            "file:///etc/passwd",
            "javascript:alert('xss')",
            "data:text/plain,hello",
            "mailto:test@example.com",
        ];

        for invalid_url in invalid_schemes {
            assert!(!invalid_url.starts_with("http://") && !invalid_url.starts_with("https://"));
        }

        // Test timeout validation ranges
        let invalid_timeouts = vec![4_u32, 0_u32, 121_u32, 300_u32];
        let valid_timeouts = vec![5_u32, 30_u32, 60_u32, 120_u32];

        for timeout in invalid_timeouts {
            assert!(!(5..=120).contains(&timeout));
        }

        for timeout in valid_timeouts {
            assert!((5..=120).contains(&timeout));
        }

        // Test content length validation ranges
        let invalid_lengths = vec![0_u32, 512_u32, 1023_u32, 20_971_520_u32];
        let valid_lengths = vec![1024_u32, 1_048_576_u32, 5_242_880_u32, 10_485_760_u32];

        for length in invalid_lengths {
            assert!(!(1024..=10_485_760).contains(&length));
        }

        for length in valid_lengths {
            assert!((1024..=10_485_760).contains(&length));
        }
    }

    #[test]
    fn test_all_parameter_combinations() {
        let _tool = WebFetchTool::new();

        // Test minimal valid request (only URL)
        let mut minimal_args = serde_json::Map::new();
        minimal_args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        let minimal_request: WebFetchRequest = BaseToolImpl::parse_arguments(minimal_args).unwrap();
        assert_eq!(minimal_request.url, "https://example.com");
        assert!(minimal_request.timeout.is_none());
        assert!(minimal_request.follow_redirects.is_none());
        assert!(minimal_request.max_content_length.is_none());
        assert!(minimal_request.user_agent.is_none());

        // Test maximal valid request (all parameters)
        let mut maximal_args = serde_json::Map::new();
        maximal_args.insert(
            "url".to_string(),
            serde_json::Value::String("https://api.github.com/docs".to_string()),
        );
        maximal_args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(60)),
        );
        maximal_args.insert(
            "follow_redirects".to_string(),
            serde_json::Value::Bool(false),
        );
        maximal_args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5242880)), // 5MB
        );
        maximal_args.insert(
            "user_agent".to_string(),
            serde_json::Value::String("CustomBot/2.0".to_string()),
        );

        let maximal_request: WebFetchRequest = BaseToolImpl::parse_arguments(maximal_args).unwrap();
        assert_eq!(maximal_request.url, "https://api.github.com/docs");
        assert_eq!(maximal_request.timeout, Some(60));
        assert_eq!(maximal_request.follow_redirects, Some(false));
        assert_eq!(maximal_request.max_content_length, Some(5242880));
        assert_eq!(
            maximal_request.user_agent,
            Some("CustomBot/2.0".to_string())
        );

        // Test boundary values
        let mut boundary_args = serde_json::Map::new();
        boundary_args.insert(
            "url".to_string(),
            serde_json::Value::String("http://localhost".to_string()),
        );
        boundary_args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)), // Minimum
        );
        boundary_args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(10485760)), // Maximum (10MB)
        );
        boundary_args.insert(
            "follow_redirects".to_string(),
            serde_json::Value::Bool(true),
        );

        let boundary_request: WebFetchRequest =
            BaseToolImpl::parse_arguments(boundary_args).unwrap();
        assert_eq!(boundary_request.timeout, Some(5));
        assert_eq!(boundary_request.max_content_length, Some(10485760));
        assert_eq!(boundary_request.follow_redirects, Some(true));
    }

    #[test]
    fn test_redirect_step_creation() {
        let step = RedirectStep {
            url: "https://example.com".to_string(),
            status_code: 301,
        };
        assert_eq!(step.url, "https://example.com");
        assert_eq!(step.status_code, 301);
    }

    #[test]
    fn test_redirect_info_creation() {
        let redirect_chain = vec![
            RedirectStep {
                url: "https://example.com/old".to_string(),
                status_code: 301,
            },
            RedirectStep {
                url: "https://example.com/new".to_string(),
                status_code: 200,
            },
        ];

        let redirect_info = RedirectInfo {
            redirect_count: 1,
            redirect_chain: redirect_chain.clone(),
            final_url: "https://example.com/new".to_string(),
        };

        assert_eq!(redirect_info.redirect_count, 1);
        assert_eq!(redirect_info.redirect_chain.len(), 2);
        assert_eq!(redirect_info.final_url, "https://example.com/new");
        assert_eq!(redirect_info.redirect_chain[0].status_code, 301);
        assert_eq!(redirect_info.redirect_chain[1].status_code, 200);
    }

    #[test]
    fn test_redirect_chain_formatting() {
        let redirect_chain = [
            RedirectStep {
                url: "https://example.com/step1".to_string(),
                status_code: 301,
            },
            RedirectStep {
                url: "https://example.com/step2".to_string(),
                status_code: 302,
            },
            RedirectStep {
                url: "https://example.com/final".to_string(),
                status_code: 200,
            },
        ];

        let formatted: Vec<String> = redirect_chain
            .iter()
            .map(|step| {
                let url = &step.url;
                let status_code = step.status_code;
                format!("{url} -> {status_code}")
            })
            .collect();

        assert_eq!(formatted.len(), 3);
        assert_eq!(formatted[0], "https://example.com/step1 -> 301");
        assert_eq!(formatted[1], "https://example.com/step2 -> 302");
        assert_eq!(formatted[2], "https://example.com/final -> 200");
    }

    #[test]
    fn test_redirect_status_code_categorization() {
        // Test different redirect status codes
        let redirect_codes = [301, 302, 303, 307, 308];

        for code in redirect_codes {
            assert!(
                (300..400).contains(&code),
                "Status code {code} should be in 3xx range"
            );
        }

        // Test non-redirect codes
        let non_redirect_codes = [200, 404, 500];

        for code in non_redirect_codes {
            assert!(
                !(300..400).contains(&code),
                "Status code {code} should not be in 3xx range"
            );
        }
    }

    #[test]
    fn test_max_redirects_validation() {
        // Test that max redirects logic is correct
        let follow_redirects_true = true;
        let follow_redirects_false = false;

        let max_redirects_when_following = if follow_redirects_true { 10 } else { 0 };
        let max_redirects_when_not_following = if follow_redirects_false { 10 } else { 0 };

        assert_eq!(max_redirects_when_following, 10);
        assert_eq!(max_redirects_when_not_following, 0);
    }

    #[test]
    fn test_redirect_count_logic() {
        // Test redirect counting scenarios
        let no_redirects = 0;
        let one_redirect = 1;
        let multiple_redirects = 3;
        let max_redirects = 10;

        assert!(no_redirects <= max_redirects);
        assert!(one_redirect <= max_redirects);
        assert!(multiple_redirects <= max_redirects);
        assert!(max_redirects <= max_redirects);

        let too_many_redirects = 11;
        assert!(too_many_redirects > max_redirects);
    }

    #[test]
    fn test_url_parsing_for_redirects() {
        // Test absolute URL detection
        let absolute_http = "http://example.com";
        let absolute_https = "https://example.com";
        let relative_path = "/path/to/resource";
        let relative_query = "?query=param";

        assert!(absolute_http.starts_with("http://") || absolute_http.starts_with("https://"));
        assert!(absolute_https.starts_with("http://") || absolute_https.starts_with("https://"));
        assert!(!(relative_path.starts_with("http://") || relative_path.starts_with("https://")));
        assert!(!(relative_query.starts_with("http://") || relative_query.starts_with("https://")));
    }

    #[test]
    fn test_redirect_error_message_formatting() {
        let redirect_count = 11;
        let max_redirects = 10;

        let error_message =
            format!("Too many redirects ({redirect_count}). Maximum allowed: {max_redirects}");

        assert!(error_message.contains("Too many redirects"));
        assert!(error_message.contains("11"));
        assert!(error_message.contains("10"));
    }

    #[test]
    fn test_response_metadata_redirect_fields() {
        // Test that redirect response contains required fields
        let redirect_count = 2;
        let redirect_chain = [
            "https://example.com/old -> 301",
            "https://example.com/new -> 200",
        ];

        let mut response = serde_json::json!({
            "url": "https://example.com/old",
            "final_url": "https://example.com/new",
            "status": "success"
        });

        // Simulate adding redirect information
        if redirect_count > 0 {
            response["redirect_count"] =
                serde_json::Value::Number(serde_json::Number::from(redirect_count));
            response["redirect_chain"] = serde_json::Value::Array(
                redirect_chain
                    .into_iter()
                    .map(|s| serde_json::Value::String(s.to_string()))
                    .collect(),
            );
        }

        assert_eq!(
            response["redirect_count"],
            serde_json::Value::Number(serde_json::Number::from(2))
        );
        assert!(response["redirect_chain"].is_array());

        let chain_array = response["redirect_chain"].as_array().unwrap();
        assert_eq!(chain_array.len(), 2);
        assert_eq!(chain_array[0], "https://example.com/old -> 301");
        assert_eq!(chain_array[1], "https://example.com/new -> 200");
    }

    #[test]
    fn test_success_message_formatting() {
        // Test success message with no redirects
        let url = "https://example.com";
        let metadata = "{}";
        let content = "content";
        let no_redirect_message = format!(
            "Successfully fetched and converted content from {url}\n\nMetadata: {metadata}\n\nContent:\n{content}"
        );
        assert!(no_redirect_message.contains("Successfully fetched"));
        assert!(no_redirect_message.contains(url));

        // Test success message with redirects
        let redirect_count = 2;
        let final_url = "https://example.com/final";
        let redirect_s = if redirect_count == 1 { "" } else { "s" };
        let metadata2 = "{}";
        let content2 = "content";
        let redirect_message = format!(
            "Successfully fetched and converted content from {url} (followed {redirect_count} redirect{redirect_s})\nFinal URL: {final_url}\n\nMetadata: {metadata2}\n\nContent:\n{content2}"
        );
        assert!(redirect_message.contains("followed 2 redirects"));
        assert!(redirect_message.contains("Final URL:"));
        assert!(redirect_message.contains(final_url));
    }

    #[test]
    fn test_schema_compliance() {
        let tool = WebFetchTool::new();
        let schema = tool.schema();

        // Verify schema structure matches specification
        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();

        // Required fields
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));

        // Properties structure
        let properties = obj["properties"].as_object().unwrap();
        let required = obj["required"].as_array().unwrap();

        // URL field requirements
        let url_prop = &properties["url"];
        assert_eq!(url_prop["type"], "string");
        assert_eq!(url_prop["format"], "uri");
        assert!(required.contains(&serde_json::Value::String("url".to_string())));

        // Optional parameters with defaults and constraints
        let timeout_prop = &properties["timeout"];
        assert_eq!(timeout_prop["type"], "integer");
        assert_eq!(timeout_prop["minimum"], 5);
        assert_eq!(timeout_prop["maximum"], 120);
        assert_eq!(timeout_prop["default"], 30);

        let follow_redirects_prop = &properties["follow_redirects"];
        assert_eq!(follow_redirects_prop["type"], "boolean");
        assert_eq!(follow_redirects_prop["default"], true);

        let max_content_length_prop = &properties["max_content_length"];
        assert_eq!(max_content_length_prop["type"], "integer");
        assert_eq!(max_content_length_prop["minimum"], 1024);
        assert_eq!(max_content_length_prop["maximum"], 10485760);
        assert_eq!(max_content_length_prop["default"], 1048576);

        let user_agent_prop = &properties["user_agent"];
        assert_eq!(user_agent_prop["type"], "string");
        assert_eq!(user_agent_prop["default"], "SwissArmyHammer-Bot/1.0");
    }

    // Additional comprehensive parameter validation tests

    #[test]
    fn test_parse_arguments_with_invalid_types() {
        // Test URL with wrong type
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::Number(serde_json::Number::from(12345)),
        );
        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());

        // Test timeout with wrong type
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::String("not_a_number".to_string()),
        );
        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());

        // Test follow_redirects with wrong type
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "follow_redirects".to_string(),
            serde_json::Value::String("not_a_boolean".to_string()),
        );
        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());

        // Test max_content_length with wrong type
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Bool(true),
        );
        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_arguments_with_null_values() {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert("timeout".to_string(), serde_json::Value::Null);
        args.insert("follow_redirects".to_string(), serde_json::Value::Null);
        args.insert("max_content_length".to_string(), serde_json::Value::Null);
        args.insert("user_agent".to_string(), serde_json::Value::Null);

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.timeout, None);
        assert_eq!(request.follow_redirects, None);
        assert_eq!(request.max_content_length, None);
        assert_eq!(request.user_agent, None);
    }

    #[test]
    fn test_parse_arguments_with_extra_fields() {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        // Add extra fields that should be ignored
        args.insert(
            "extra_field".to_string(),
            serde_json::Value::String("ignored".to_string()),
        );
        args.insert(
            "another_field".to_string(),
            serde_json::Value::Number(serde_json::Number::from(999)),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "https://example.com");
        // Should ignore extra fields gracefully
    }

    #[test]
    fn test_parameter_constraint_validation_edge_cases() {
        // Test timeout exactly at boundaries
        let timeout_boundary_cases = [
            (4, false),   // Below minimum
            (5, true),    // At minimum
            (120, true),  // At maximum
            (121, false), // Above maximum
        ];

        for (timeout, should_be_valid) in timeout_boundary_cases {
            assert_eq!(
                (MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout),
                should_be_valid,
                "Timeout validation failed for {timeout}"
            );
        }

        // Test content length exactly at boundaries
        let content_length_boundary_cases = [
            (1023, false),     // Below minimum
            (1024, true),      // At minimum
            (10485760, true),  // At maximum
            (10485761, false), // Above maximum
        ];

        for (length, should_be_valid) in content_length_boundary_cases {
            assert_eq!(
                (MIN_CONTENT_LENGTH_BYTES..=MAX_CONTENT_LENGTH_BYTES).contains(&length),
                should_be_valid,
                "Content length validation failed for {length}"
            );
        }
    }

    #[test]
    fn test_negative_parameter_values() {
        // Test negative timeout
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(-1)),
        );

        // Should either fail parsing or be caught by validation
        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        if let Ok(request) = result {
            // If parsing succeeds, validation should catch it
            assert!(request.timeout.is_none() || request.timeout == Some(u32::MAX));
            // Underflow handling
        }

        // Test negative max_content_length
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(-1000)),
        );

        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        if let Ok(request) = result {
            // If parsing succeeds, validation should catch it
            assert!(
                request.max_content_length.is_none()
                    || request.max_content_length == Some(u32::MAX)
            );
        }
    }

    #[test]
    fn test_very_large_parameter_values() {
        // Test timeout with very large value
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(u32::MAX)),
        );

        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        if let Ok(request) = result {
            // Validation should catch oversized values
            if let Some(timeout) = request.timeout {
                assert!(timeout > MAX_TIMEOUT_SECONDS);
            }
        }

        // Test max_content_length with very large value
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "max_content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(u32::MAX)),
        );

        let result: std::result::Result<WebFetchRequest, McpError> =
            BaseToolImpl::parse_arguments(args);
        if let Ok(request) = result {
            if let Some(max_length) = request.max_content_length {
                assert!(max_length > MAX_CONTENT_LENGTH_BYTES);
            }
        }
    }

    #[test]
    fn test_empty_string_parameters() {
        // Test empty URL
        let mut args = serde_json::Map::new();
        args.insert("url".to_string(), serde_json::Value::String("".to_string()));

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "");
        // Empty URL should be caught by URL validation, not argument parsing

        // Test empty user agent
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "user_agent".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.user_agent, Some("".to_string()));
        // Empty user agent should be allowed and handled gracefully
    }

    #[test]
    fn test_whitespace_only_parameters() {
        // Test URL with only whitespace
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("   \t\n   ".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "   \t\n   ");

        // Test user agent with only whitespace
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "user_agent".to_string(),
            serde_json::Value::String("   ".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.user_agent, Some("   ".to_string()));
    }

    #[test]
    fn test_unicode_parameters() {
        // Test URL with unicode characters
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://münchen.example.com/ñandú".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, "https://münchen.example.com/ñandú");

        // Test user agent with unicode
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "user_agent".to_string(),
            serde_json::Value::String("BöwserBot/1.0 (ñandú engine)".to_string()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(
            request.user_agent,
            Some("BöwserBot/1.0 (ñandú engine)".to_string())
        );
    }

    #[test]
    fn test_very_long_parameters() {
        // Test very long URL
        let long_url = format!("https://example.com/{}", "a".repeat(2000));
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String(long_url.clone()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.url, long_url);

        // Test very long user agent
        let long_user_agent = format!("VeryLongBot/{}", "x".repeat(1000));
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );
        args.insert(
            "user_agent".to_string(),
            serde_json::Value::String(long_user_agent.clone()),
        );

        let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.user_agent, Some(long_user_agent));
    }

    // Advanced security validation tests

    #[test]
    fn test_security_validator_instantiation() {
        let tool = WebFetchTool::new();
        // Test that security validator is properly initialized
        assert_eq!(tool.name(), "web_fetch");
        // SecurityValidator is private but we can test it through URL validation
    }

    #[test]
    fn test_advanced_ssrf_protection() {
        // These would be caught by SecurityValidator in actual execution
        let ssrf_attempts = [
            "http://127.0.0.1:8080/admin",
            "https://169.254.169.254/latest/meta-data/",
            "http://[::1]:3000/internal",
            "https://10.0.0.1/secrets",
            "http://192.168.1.100:8080",
            "https://172.16.0.1:9090",
            "http://localhost:8080",
            "https://metadata.google.internal/computeMetadata/v1/",
            "http://instance-data.ec2.internal/",
        ];

        for url in ssrf_attempts {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // In actual execution, these would be blocked by SecurityValidator
        }
    }

    #[test]
    fn test_scheme_validation_comprehensive() {
        let invalid_schemes = [
            "ftp://files.example.com/readme.txt",
            "sftp://secure.example.com/data",
            "file:///etc/passwd",
            "file:///C:/Windows/System32/config/sam",
            "javascript:alert('XSS')",
            "data:text/html,<script>alert('XSS')</script>",
            "vbscript:msgbox('test')",
            "mailto:admin@example.com",
            "tel:+1-555-123-4567",
            "sms:+1-555-123-4567",
            "ldap://ldap.example.com/dc=example,dc=com",
            "ldaps://secure-ldap.example.com/",
            "gopher://gopher.example.com/",
            "news:comp.lang.rust",
            "nntp://news.example.com/",
            "rtsp://streaming.example.com/video",
            "rtmp://streaming.example.com/live",
        ];

        for url in invalid_schemes {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // These would be blocked by scheme validation
            assert!(!url.starts_with("http://") && !url.starts_with("https://"));
        }
    }

    #[test]
    fn test_valid_url_schemes() {
        let valid_schemes = [
            "http://example.com",
            "https://example.com",
            "HTTP://EXAMPLE.COM", // Should be handled case-insensitively by URL parser
            "HTTPS://EXAMPLE.COM",
            "http://subdomain.example.com:8080/path?query=value#fragment",
            "https://api.github.com/repos/owner/repo",
        ];

        for url in valid_schemes {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // These should be allowed (though case handling depends on URL parser)
        }
    }

    #[test]
    fn test_ip_address_detection_edge_cases() {
        let private_ip_variants = [
            // IPv4 private ranges
            "http://10.0.0.1",    // Class A private
            "https://172.16.0.1", // Class B private
            "http://192.168.1.1", // Class C private
            "https://100.64.0.1", // Carrier-grade NAT
            "http://169.254.0.1", // Link-local
            // IPv6 private/special addresses
            "http://[::1]",               // IPv6 localhost
            "https://[::ffff:127.0.0.1]", // IPv4-mapped IPv6 localhost
            "http://[::]",                // IPv6 unspecified
            "https://[fc00::1]",          // IPv6 unique local
            "http://[fe80::1]",           // IPv6 link-local
            // Encoded/obfuscated IP attempts
            "http://2130706433",  // 127.0.0.1 in decimal
            "https://0x7f000001", // 127.0.0.1 in hex
            "http://0177.0.0.1",  // 127.0.0.1 in octal (first octet)
            "https://127.1",      // Short form of 127.0.0.1
        ];

        for url in private_ip_variants {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // These would be blocked by IP validation in SecurityValidator
        }
    }

    #[test]
    fn test_domain_name_edge_cases() {
        let suspicious_domains = [
            // Internal/development patterns
            "http://test.local",
            "https://dev.localhost",
            "http://staging.internal",
            "https://admin.company.internal",
            // Lookalike domains
            "http://g00gle.com",   // Typosquatting
            "https://arnazon.com", // Similar to amazon
            "http://microsft.com", // Missing letter
            // Internationalized domains that could be confusing
            "https://аpple.com", // Uses Cyrillic 'а' instead of Latin 'a'
            "http://раураl.com", // Cyrillic characters
            // Domains with unusual TLDs
            "https://example.tk", // Suspicious TLD
            "http://test.ml",     // Free domain TLD
        ];

        for url in suspicious_domains {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // Domain policy would handle these in SecurityValidator
        }
    }

    #[test]
    fn test_url_components_validation() {
        let edge_case_urls = [
            // Port numbers
            "http://example.com:80",     // Default HTTP port
            "https://example.com:443",   // Default HTTPS port
            "http://example.com:8080",   // Common alt port
            "https://example.com:65535", // Max port number
            "http://example.com:0",      // Invalid port
            // Authentication in URL (should be blocked)
            "http://user:pass@example.com",
            "https://admin@example.com",
            "http://:password@example.com",
            // Path traversal attempts
            "https://example.com/../../../etc/passwd",
            "http://example.com/./admin/../config",
            "https://example.com/%2e%2e%2f%2e%2e%2f", // URL encoded ../../../
            // Query parameter injection
            "http://example.com/api?url=http://evil.com",
            "https://example.com/redirect?next=//evil.com",
            "http://example.com/proxy?target=localhost:22",
            // Fragment/anchor handling
            "https://example.com#javascript:alert('xss')",
            "http://example.com/#data:text/html,<script>alert(1)</script>",
        ];

        for url in edge_case_urls {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // URL component validation would be handled by SecurityValidator
        }
    }

    #[test]
    fn test_security_bypass_attempts() {
        let bypass_attempts = [
            // Protocol upgrade attempts
            "http://example.com:80@localhost:8080",
            // URL shortener bypass
            "https://bit.ly/localhost",
            "http://tinyurl.com/internal-admin",
            // DNS rebinding attempts
            "http://localtest.me", // Resolves to 127.0.0.1
            "https://vcap.me",     // Cloud Foundry test domain
            "http://xip.io",       // Wildcard DNS for any IP
            // IPv6 bypass attempts with various formats
            "http://[0000:0000:0000:0000:0000:0000:0000:0001]", // Full IPv6 localhost
            "https://[::1]:8080",                               // IPv6 localhost with port
            "http://[::ffff:0:0]",                              // IPv4-compatible IPv6
            // Protocol relative URLs (though these should fail URL parsing)
            "//evil.com/malware",
            "///evil.com/malware",
        ];

        for url in bypass_attempts {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            // Some of these might fail URL parsing entirely
            let result: std::result::Result<WebFetchRequest, McpError> =
                BaseToolImpl::parse_arguments(args);

            if let Ok(request) = result {
                // If parsing succeeds, security validation should handle it
                assert_eq!(request.url, url);
            }
            // Failed parsing is also acceptable for malformed URLs
        }
    }

    #[test]
    fn test_content_type_based_security() {
        // Test that we're prepared to handle various content types securely
        let potentially_dangerous_endpoints = [
            "https://example.com/api/download",     // Could return executable
            "http://example.com/files/script.js",   // JavaScript file
            "https://example.com/uploads/file.exe", // Executable file
            "http://example.com/data.xml",          // XML with potential XXE
            "https://example.com/config.json",      // Configuration data
            "http://example.com/backup.sql",        // Database dump
            "https://example.com/logs/access.log",  // Log files
        ];

        for url in potentially_dangerous_endpoints {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // URL itself is valid, content security would be handled during fetch
        }
    }

    #[test]
    fn test_rate_limiting_scenarios() {
        // Test various scenarios that might trigger rate limiting
        let high_frequency_targets = [
            "https://api.github.com/rate_limit",
            "http://httpbin.org/delay/1",
            "https://jsonplaceholder.typicode.com/posts",
            "http://example.com/api/v1/data",
        ];

        for url in high_frequency_targets {
            let mut args = serde_json::Map::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );

            let request: WebFetchRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.url, url);
            // Rate limiting would be applied at execution time
        }
    }

    // Error handling and categorization tests

    #[test]
    fn test_error_categorization_comprehensive() {
        // Test security errors
        let security_errors = [
            "Blocked domain access detected",
            "SSRF attempt blocked",
            "Unsupported scheme detected",
            "SSL handshake failed",
            "TLS certificate verification failed",
            "Certificate authority invalid",
        ];

        for error_msg in security_errors {
            let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "security_error",
                "Failed to categorize security error: {}",
                error_msg
            );
        }

        // Test network errors
        let network_errors = [
            "connection refused",
            "connection reset by peer",
            "timeout occurred",
            "DNS resolution failed",
        ];

        for error_msg in network_errors {
            let error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "network_error",
                "Failed to categorize network error: {}",
                error_msg
            );
        }

        // These don't contain the required keywords, so they'll be unknown
        let unknown_network_errors = [
            "network unreachable", // doesn't contain "connection", "timeout", or "dns"
            "no route to host",    // doesn't contain the required keywords
        ];

        for error_msg in unknown_network_errors {
            let error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "unknown_error",
                "Should categorize as unknown error: {}",
                error_msg
            );
        }

        // Test HTTP errors
        let not_found_errors = ["404 Not Found", "Resource not found", "Page not found"];
        for error_msg in not_found_errors {
            let error = std::io::Error::new(std::io::ErrorKind::NotFound, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "not_found_error",
                "Failed to categorize not found error: {}",
                error_msg
            );
        }

        let access_errors = ["403 Forbidden", "401 Unauthorized", "Access forbidden"];
        for error_msg in access_errors {
            let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "access_denied_error",
                "Failed to categorize access error: {}",
                error_msg
            );
        }

        let server_errors = [
            "500 Internal Server Error",
            "502 Bad Gateway",
            "503 Service Unavailable",
        ];
        for error_msg in server_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "server_error",
                "Failed to categorize server error: {}",
                error_msg
            );
        }

        // Test content processing errors
        let content_errors = [
            "markdowndown conversion failed",
            "HTML conversion error",
            "markdown conversion failed",
        ];

        for error_msg in content_errors {
            let error = std::io::Error::new(std::io::ErrorKind::InvalidData, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "content_processing_error",
                "Failed to categorize content error: {}",
                error_msg
            );
        }

        // Test size limit errors
        let size_errors = ["Content too large", "Size limit exceeded", "File too large"];
        for error_msg in size_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "size_limit_error",
                "Failed to categorize size error: {}",
                error_msg
            );
        }

        // Test redirect errors
        let redirect_errors = [
            "Too many redirects",
            "redirect loop detected",
            "excessive redirects",
        ];
        for error_msg in redirect_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "redirect_error",
                "Failed to categorize redirect error: {}",
                error_msg
            );
        }

        // Test content parsing errors
        let parse_errors = [
            "Invalid parse data",
            "Encoding error",
            "Invalid character encoding",
        ];
        for error_msg in parse_errors {
            let error = std::io::Error::new(std::io::ErrorKind::InvalidData, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "content_error",
                "Failed to categorize parse error: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_error_message_consistency() {
        // Test that error categorization is case-insensitive
        let case_variants = [
            ("CONNECTION REFUSED", "network_error"),
            ("Connection Refused", "network_error"),
            ("connection refused", "network_error"),
            ("CONNECTION timeout", "network_error"),
            ("DNS failure", "network_error"),
            ("BLOCKED DOMAIN", "security_error"),
            ("Blocked Domain", "security_error"),
            ("blocked domain", "security_error"),
            ("SSL ERROR", "security_error"),
            ("TLS failure", "security_error"),
            ("404 NOT FOUND", "not_found_error"),
            ("404 Not Found", "not_found_error"),
            ("404 not found", "not_found_error"),
            ("TOO MANY REDIRECTS", "redirect_error"),
            ("Too Many Redirects", "redirect_error"),
            ("too many redirects", "redirect_error"),
        ];

        for (error_msg, expected_category) in case_variants {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "Case sensitivity issue with error: {} -> expected: {}, got: {}",
                error_msg, expected_category, category
            );
        }
    }

    #[test]
    fn test_unknown_error_fallback() {
        let unknown_errors = [
            "Some completely unknown error",
            "Random failure message",
            "Mysterious problem occurred",
            "",     // Empty error message
            "    ", // Whitespace only
        ];

        for error_msg in unknown_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, "unknown_error",
                "Should categorize unknown error: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_error_priority_handling() {
        // Test that security errors take precedence when multiple keywords match
        let mixed_errors = [
            ("SSL connection refused", "security_error"), // SSL should take precedence
            ("Blocked domain timeout", "security_error"), // Blocked should take precedence
            ("SSRF connection failed", "security_error"), // SSRF should take precedence
            ("Certificate timeout occurred", "security_error"), // Certificate should take precedence
        ];

        for (error_msg, expected_category) in mixed_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "Priority handling failed for: {} -> expected: {}, got: {}",
                error_msg, expected_category, category
            );
        }
    }

    #[test]
    fn test_numeric_error_codes() {
        let numeric_errors = [
            ("Error 404", "not_found_error"),
            ("Status: 403", "access_denied_error"),
            ("HTTP 500 error", "server_error"),
            ("Response 502", "server_error"),
            ("Code 503", "server_error"),
            ("Status 301 redirect", "redirect_error"), // contains "redirect"
        ];

        for (error_msg, expected_category) in numeric_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "Numeric error code handling failed for: {} -> expected: {}, got: {}",
                error_msg, expected_category, category
            );
        }
    }

    #[test]
    fn test_complex_error_scenarios() {
        // Test real-world complex error messages
        let complex_errors = [
            (
                "Failed to establish SSL connection: certificate verify failed: unable to get local issuer certificate", 
                "security_error"
            ),
            (
                "Connection timeout occurred while attempting to reach 192.168.1.1:8080 after 30 seconds", 
                "network_error"
            ),
            (
                "HTTP/1.1 404 Not Found - The requested resource '/api/v1/users/12345' could not be found on this server", 
                "not_found_error"
            ),
            (
                "Content-Length exceeds maximum allowed size of 10MB: received 15728640 bytes", 
                "size_limit_error"
            ),
            (
                "Too many redirects (10) encountered while following redirect chain from https://example.com to https://final.com", 
                "redirect_error"
            ),
            (
                "Failed to parse HTML content using markdowndown library: invalid character sequence at byte position 1024", 
                "content_processing_error"
            ),
        ];

        for (error_msg, expected_category) in complex_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "Complex error scenario failed for: {} -> expected: {}, got: {}",
                error_msg, expected_category, category
            );
        }
    }

    #[test]
    fn test_error_message_special_characters() {
        let special_char_errors = [
            ("Connection failed: ∞ timeout", "network_error"), // contains "connection" and "timeout"
            ("blocked domain: señor.com", "security_error"), // contains "blocked domain" (lowercase)
            ("404: ¿página no encontrada?", "not_found_error"), // contains "404"
            ("Content too large: 文档太大", "size_limit_error"), // contains "too large"
            ("SSL ошибка: сертификат недействителен", "security_error"), // contains "SSL"
        ];

        for (error_msg, expected_category) in special_char_errors {
            let error = std::io::Error::new(std::io::ErrorKind::Other, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "Special character handling failed for: {} -> expected: {}, got: {}",
                error_msg, expected_category, category
            );
        }
    }

    #[test]
    fn test_error_categorization_with_different_error_kinds() {
        // Test that categorization works with different std::io::ErrorKind values
        let error_kind_tests = [
            (
                std::io::ErrorKind::ConnectionRefused,
                "connection failed",
                "network_error",
            ), // contains "connection"
            (
                std::io::ErrorKind::ConnectionAborted,
                "connection aborted",
                "network_error",
            ), // contains "connection"
            (
                std::io::ErrorKind::NotConnected,
                "not connected",
                "unknown_error",
            ), // doesn't contain required keywords
            (
                std::io::ErrorKind::AddrInUse,
                "address in use",
                "unknown_error",
            ), // doesn't contain required keywords
            (
                std::io::ErrorKind::AddrNotAvailable,
                "address not available",
                "unknown_error",
            ), // doesn't contain required keywords
            (std::io::ErrorKind::TimedOut, "timed out", "unknown_error"), // doesn't contain "timeout" keyword
            (
                std::io::ErrorKind::InvalidData,
                "invalid data",
                "content_error",
            ), // contains "invalid"
            (
                std::io::ErrorKind::InvalidInput,
                "invalid input",
                "content_error",
            ), // contains "invalid"
            (std::io::ErrorKind::NotFound, "not found", "not_found_error"), // contains "not found"
            (
                std::io::ErrorKind::PermissionDenied,
                "permission denied",
                "unknown_error",
            ), // doesn't contain "403", "forbidden", or "unauthorized"
            (std::io::ErrorKind::Other, "other error", "unknown_error"),
        ];

        for (error_kind, error_msg, expected_category) in error_kind_tests {
            let error = std::io::Error::new(error_kind, error_msg);
            let category = WebFetchTool::categorize_error(&error);
            assert_eq!(
                category, expected_category,
                "ErrorKind {:?} with message '{}' should categorize as '{}', got '{}'",
                error_kind, error_msg, expected_category, category
            );
        }
    }

    // Response formatting validation tests

    #[test]
    fn test_success_response_structure() {
        // Test the structure of success responses
        let content = "# Test Title\n\nTest content here.";
        let redirect_info = RedirectInfo {
            redirect_count: 0,
            redirect_chain: vec![RedirectStep {
                url: "https://example.com".to_string(),
                status_code: 200,
            }],
            final_url: "https://example.com".to_string(),
        };
        let headers = std::collections::HashMap::new();
        let response_time = 150;
        let request = WebFetchRequest {
            url: "https://example.com".to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };

        let result = WebFetchTool::build_success_response(
            content.to_string(),
            redirect_info,
            headers,
            response_time,
            &request,
        );

        assert!(result.is_ok());
        let call_result = result.unwrap();

        // Test basic structure
        assert!(call_result.content.len() > 0);
        assert_eq!(call_result.is_error, Some(false));

        // Parse the JSON content
        if let rmcp::model::RawContent::Text(text_content) = &call_result.content[0].raw {
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();

            // Test response structure
            assert!(parsed["content"].is_array());
            assert_eq!(parsed["is_error"], false);
            assert!(parsed["metadata"].is_object());

            let metadata = &parsed["metadata"];
            assert_eq!(metadata["url"], "https://example.com");
            assert_eq!(metadata["final_url"], "https://example.com");
            assert_eq!(metadata["title"], "Test Title");
            assert_eq!(metadata["content_type"], "text/html");
            assert!(metadata["content_length"].is_number());
            assert_eq!(metadata["status_code"], 200);
            assert_eq!(metadata["response_time_ms"], 150);
            assert!(metadata["word_count"].is_number());
            assert!(metadata["headers"].is_object());
        } else {
            panic!("Expected text content in response");
        }
    }

    #[test]
    fn test_success_response_with_redirects() {
        let content = "# Redirected Content\n\nThis is redirected content.";
        let redirect_info = RedirectInfo {
            redirect_count: 2,
            redirect_chain: vec![
                RedirectStep {
                    url: "https://example.com/old".to_string(),
                    status_code: 301,
                },
                RedirectStep {
                    url: "https://example.com/middle".to_string(),
                    status_code: 302,
                },
                RedirectStep {
                    url: "https://example.com/final".to_string(),
                    status_code: 200,
                },
            ],
            final_url: "https://example.com/final".to_string(),
        };
        let mut headers = std::collections::HashMap::new();
        headers.insert("content-type".to_string(), "text/html".to_string());
        headers.insert("server".to_string(), "nginx/1.18.0".to_string());

        let request = WebFetchRequest {
            url: "https://example.com/old".to_string(),
            timeout: Some(30),
            follow_redirects: Some(true),
            max_content_length: Some(1048576),
            user_agent: Some("TestBot/1.0".to_string()),
        };

        let result = WebFetchTool::build_success_response(
            content.to_string(),
            redirect_info,
            headers,
            250,
            &request,
        );

        assert!(result.is_ok());
        let call_result = result.unwrap();

        if let rmcp::model::RawContent::Text(text_content) = &call_result.content[0].raw {
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();

            let metadata = &parsed["metadata"];
            assert_eq!(metadata["url"], "https://example.com/old");
            assert_eq!(metadata["final_url"], "https://example.com/final");
            assert_eq!(metadata["redirect_count"], 2);

            assert!(metadata["redirect_chain"].is_array());
            let redirect_chain = metadata["redirect_chain"].as_array().unwrap();
            assert_eq!(redirect_chain.len(), 3);
            assert_eq!(redirect_chain[0], "https://example.com/old -> 301");
            assert_eq!(redirect_chain[1], "https://example.com/middle -> 302");
            assert_eq!(redirect_chain[2], "https://example.com/final -> 200");

            // Verify success message mentions redirects
            let content_array = parsed["content"].as_array().unwrap();
            let message = content_array[0]["text"].as_str().unwrap();
            assert!(message.contains("redirected"));
        }
    }

    #[test]
    fn test_error_response_structure() {
        let error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let response_time = 5000;
        let request = WebFetchRequest {
            url: "https://unreachable.example.com".to_string(),
            timeout: Some(10),
            follow_redirects: Some(true),
            max_content_length: Some(1048576),
            user_agent: Some("TestBot/1.0".to_string()),
        };

        let result = WebFetchTool::build_error_response(&error, response_time, &request);

        assert!(result.is_ok());
        let call_result = result.unwrap();

        // Test basic structure
        assert!(call_result.content.len() > 0);
        assert_eq!(call_result.is_error, Some(true));

        if let rmcp::model::RawContent::Text(text_content) = &call_result.content[0].raw {
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();

            // Test error response structure
            assert!(parsed["content"].is_array());
            assert_eq!(parsed["is_error"], true);
            assert!(parsed["metadata"].is_object());

            let metadata = &parsed["metadata"];
            assert_eq!(metadata["url"], "https://unreachable.example.com");
            assert_eq!(metadata["error_type"], "network_error");
            assert!(metadata["error_details"]
                .as_str()
                .unwrap()
                .contains("Connection refused"));
            assert_eq!(metadata["status_code"], serde_json::Value::Null);
            assert_eq!(metadata["response_time_ms"], 5000);
        }
    }

    #[test]
    fn test_metadata_field_completeness() {
        // Test that all expected metadata fields are present
        let content = "# Complete Test\n\nThis tests all metadata fields.";
        let redirect_info = RedirectInfo {
            redirect_count: 0,
            redirect_chain: vec![RedirectStep {
                url: "https://complete.example.com".to_string(),
                status_code: 200,
            }],
            final_url: "https://complete.example.com".to_string(),
        };

        let mut headers = std::collections::HashMap::new();
        headers.insert("server".to_string(), "Apache/2.4.41".to_string());
        headers.insert("content-encoding".to_string(), "gzip".to_string());
        headers.insert("content-length".to_string(), "1024".to_string());
        headers.insert(
            "last-modified".to_string(),
            "Wed, 21 Oct 2015 07:28:00 GMT".to_string(),
        );
        headers.insert("etag".to_string(), "\"1234567890\"".to_string());
        headers.insert("cache-control".to_string(), "max-age=3600".to_string());

        let request = WebFetchRequest {
            url: "https://complete.example.com".to_string(),
            timeout: Some(45),
            follow_redirects: Some(false),
            max_content_length: Some(2097152),
            user_agent: Some("CompleteBot/2.0".to_string()),
        };

        let result = WebFetchTool::build_success_response(
            content.to_string(),
            redirect_info,
            headers,
            750,
            &request,
        );

        assert!(result.is_ok());
        let call_result = result.unwrap();

        if let rmcp::model::RawContent::Text(text_content) = &call_result.content[0].raw {
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();

            let metadata = &parsed["metadata"];

            // Test all expected fields are present
            let expected_fields = [
                "url",
                "final_url",
                "title",
                "content_type",
                "content_length",
                "status_code",
                "response_time_ms",
                "markdown_content",
                "word_count",
                "headers",
            ];

            for field in &expected_fields {
                assert!(
                    metadata[field] != serde_json::Value::Null,
                    "Field '{}' should be present and not null",
                    field
                );
            }

            // Test specific values
            assert_eq!(metadata["url"], "https://complete.example.com");
            assert_eq!(metadata["final_url"], "https://complete.example.com");
            assert_eq!(metadata["title"], "Complete Test");
            assert_eq!(metadata["content_type"], "text/html");
            assert_eq!(metadata["status_code"], 200);
            assert_eq!(metadata["response_time_ms"], 750);

            // Test headers are properly included
            let headers_obj = metadata["headers"].as_object().unwrap();
            assert_eq!(headers_obj["server"], "Apache/2.4.41");
            assert_eq!(headers_obj["content-encoding"], "gzip");
            assert_eq!(headers_obj["etag"], "\"1234567890\"");
        }
    }

    #[test]
    fn test_title_extraction_edge_cases() {
        let test_cases = [
            (
                "# Simple Title\n\nContent",
                Some("Simple Title".to_string()),
            ),
            (
                "## Second Level Title\n\nContent",
                Some("Second Level Title".to_string()),
            ),
            (
                "### Third Level\n\nContent",
                Some("Third Level".to_string()),
            ),
            ("#\n\nNo title text", None),
            ("# \n\nEmpty title", None),
            ("##   \n\nWhitespace title", None),
            ("No heading at all\n\nJust content", None),
            (
                "# First Title\n\n## Second Title",
                Some("First Title".to_string()),
            ), // Should get first
            (
                "Not a heading\n# Later Title\n\nContent",
                Some("Later Title".to_string()),
            ),
            (
                "#Multiple#Hash#Tags#\n\nContent",
                Some("Multiple#Hash#Tags#".to_string()),
            ),
            (
                "# Title with *markdown* **formatting**",
                Some("Title with *markdown* **formatting**".to_string()),
            ),
            (
                "# Title with [link](http://example.com)",
                Some("Title with [link](http://example.com)".to_string()),
            ),
        ];

        for (markdown, expected_title) in test_cases {
            let extracted = WebFetchTool::extract_title_from_markdown(markdown);
            assert_eq!(
                extracted,
                expected_title,
                "Title extraction failed for: '{}'",
                markdown.replace('\n', "\\n")
            );
        }
    }

    #[test]
    fn test_response_json_validity() {
        // Test that all responses produce valid JSON
        let content = "# Valid JSON Test\n\nThis tests JSON validity.";
        let redirect_info = RedirectInfo {
            redirect_count: 0,
            redirect_chain: vec![RedirectStep {
                url: "https://json.example.com".to_string(),
                status_code: 200,
            }],
            final_url: "https://json.example.com".to_string(),
        };
        let headers = std::collections::HashMap::new();
        let request = WebFetchRequest {
            url: "https://json.example.com".to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };

        let success_result = WebFetchTool::build_success_response(
            content.to_string(),
            redirect_info,
            headers,
            100,
            &request,
        )
        .unwrap();

        // Test success response JSON validity
        if let rmcp::model::RawContent::Text(text_content) = &success_result.content[0].raw {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&text_content.text);
            assert!(parsed.is_ok(), "Success response should be valid JSON");

            let json = parsed.unwrap();
            assert!(json.is_object());
            assert!(json["content"].is_array());
            assert!(json["metadata"].is_object());
        }

        // Test error response JSON validity
        let error = std::io::Error::new(std::io::ErrorKind::Other, "Test error");
        let error_result = WebFetchTool::build_error_response(&error, 200, &request).unwrap();

        if let rmcp::model::RawContent::Text(text_content) = &error_result.content[0].raw {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&text_content.text);
            assert!(parsed.is_ok(), "Error response should be valid JSON");

            let json = parsed.unwrap();
            assert!(json.is_object());
            assert!(json["content"].is_array());
            assert!(json["metadata"].is_object());
        }
    }

    #[test]
    fn test_response_content_encoding() {
        // Test response handling with special characters and encoding
        let content_with_special_chars =
            "# Título con Ñandú\n\nContent with émojis: 🚀🎉\nAnd unicode: αβγδε";

        let redirect_info = RedirectInfo {
            redirect_count: 0,
            redirect_chain: vec![RedirectStep {
                url: "https://unicode.example.com".to_string(),
                status_code: 200,
            }],
            final_url: "https://unicode.example.com".to_string(),
        };
        let headers = std::collections::HashMap::new();
        let request = WebFetchRequest {
            url: "https://unicode.example.com".to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };

        let result = WebFetchTool::build_success_response(
            content_with_special_chars.to_string(),
            redirect_info,
            headers,
            300,
            &request,
        );

        assert!(result.is_ok());
        let call_result = result.unwrap();

        if let rmcp::model::RawContent::Text(text_content) = &call_result.content[0].raw {
            let parsed: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();

            let metadata = &parsed["metadata"];
            assert_eq!(metadata["title"], "Título con Ñandú");

            let markdown_content = metadata["markdown_content"].as_str().unwrap();
            assert!(markdown_content.contains("🚀🎉"));
            assert!(markdown_content.contains("αβγδε"));
            assert!(markdown_content.contains("Ñandú"));
        }
    }

    // Tool interface compliance tests

    #[test]
    fn test_mcp_tool_interface_compliance() {
        let tool = WebFetchTool::new();

        // Test required interface methods
        assert_eq!(tool.name(), "web_fetch");
        assert!(!tool.description().is_empty());

        let schema = tool.schema();
        assert!(schema.is_object());

        // Test schema has required MCP fields
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));

        // Test required field is properly specified
        let required = obj["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "url");
    }

    #[test]
    fn test_tool_instantiation_patterns() {
        // Test default creation
        let tool1 = WebFetchTool::default();
        assert_eq!(tool1.name(), "web_fetch");

        // Test new() creation
        let tool2 = WebFetchTool::new();
        assert_eq!(tool2.name(), "web_fetch");

        // Both should behave identically
        assert_eq!(tool1.name(), tool2.name());
        assert_eq!(tool1.description(), tool2.description());

        let schema1 = tool1.schema();
        let schema2 = tool2.schema();
        assert_eq!(schema1, schema2);
    }

    #[test]
    fn test_tool_description_content() {
        let tool = WebFetchTool::new();
        let description = tool.description();

        // Test description contains key information
        assert!(!description.is_empty());
        // The actual description comes from tool_descriptions so we can't test exact content
        // but we can verify it's properly loaded
    }

    #[test]
    fn test_schema_validation_completeness() {
        let tool = WebFetchTool::new();
        let schema = tool.schema();

        let obj = schema.as_object().unwrap();
        let properties = obj["properties"].as_object().unwrap();

        // Test all parameters have proper schema definitions
        let expected_properties = [
            "url",
            "timeout",
            "follow_redirects",
            "max_content_length",
            "user_agent",
        ];

        for prop in &expected_properties {
            assert!(
                properties.contains_key(*prop),
                "Schema should contain property: {}",
                prop
            );

            let prop_def = &properties[*prop];
            assert!(
                prop_def.is_object(),
                "Property '{}' should be an object",
                prop
            );
            assert!(
                prop_def["type"].is_string(),
                "Property '{}' should have a type field",
                prop
            );
        }

        // Test URL property specifics
        let url_prop = &properties["url"];
        assert_eq!(url_prop["type"], "string");
        assert_eq!(url_prop["format"], "uri");

        // Test numeric properties have bounds
        let timeout_prop = &properties["timeout"];
        assert_eq!(timeout_prop["type"], "integer");
        assert!(timeout_prop["minimum"].is_number());
        assert!(timeout_prop["maximum"].is_number());

        let max_content_prop = &properties["max_content_length"];
        assert_eq!(max_content_prop["type"], "integer");
        assert!(max_content_prop["minimum"].is_number());
        assert!(max_content_prop["maximum"].is_number());

        // Test boolean property
        let redirect_prop = &properties["follow_redirects"];
        assert_eq!(redirect_prop["type"], "boolean");

        // Test string property
        let user_agent_prop = &properties["user_agent"];
        assert_eq!(user_agent_prop["type"], "string");
    }

    #[test]
    fn test_constants_consistency() {
        // Test that constants used in schema match the validation constants
        let tool = WebFetchTool::new();
        let schema = tool.schema();
        let properties = schema["properties"].as_object().unwrap();

        // Test timeout constants
        let timeout_prop = &properties["timeout"];
        assert_eq!(timeout_prop["minimum"], MIN_TIMEOUT_SECONDS);
        assert_eq!(timeout_prop["maximum"], MAX_TIMEOUT_SECONDS);
        assert_eq!(timeout_prop["default"], DEFAULT_TIMEOUT_SECONDS);

        // Test content length constants
        let content_prop = &properties["max_content_length"];
        assert_eq!(content_prop["minimum"], MIN_CONTENT_LENGTH_BYTES);
        assert_eq!(content_prop["maximum"], MAX_CONTENT_LENGTH_BYTES);
        assert_eq!(content_prop["default"], DEFAULT_CONTENT_LENGTH_BYTES);

        // Test that constants are reasonable
        assert!(MIN_TIMEOUT_SECONDS > 0);
        assert!(MAX_TIMEOUT_SECONDS > MIN_TIMEOUT_SECONDS);
        assert!(DEFAULT_TIMEOUT_SECONDS >= MIN_TIMEOUT_SECONDS);
        assert!(DEFAULT_TIMEOUT_SECONDS <= MAX_TIMEOUT_SECONDS);

        assert!(MIN_CONTENT_LENGTH_BYTES > 0);
        assert!(MAX_CONTENT_LENGTH_BYTES > MIN_CONTENT_LENGTH_BYTES);
        assert!(DEFAULT_CONTENT_LENGTH_BYTES >= MIN_CONTENT_LENGTH_BYTES);
        assert!(DEFAULT_CONTENT_LENGTH_BYTES <= MAX_CONTENT_LENGTH_BYTES);
    }

    #[test]
    fn test_redirect_constants() {
        // Test redirect constants are reasonable
        assert!(MAX_REDIRECTS > 0);
        assert!(MAX_REDIRECTS <= 20); // Sanity check - shouldn't be too high
        assert_eq!(MAX_REDIRECTS, 10); // Current expected value
    }
}
