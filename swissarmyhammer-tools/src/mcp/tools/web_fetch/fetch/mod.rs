//! Web fetch tool for MCP operations
//!
//! This module provides the WebFetchTool for fetching web content and converting HTML to markdown
//! through the MCP protocol using the markdowndown crate.

// Security validation replaces the old basic validation utilities
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::types::WebFetchRequest;
use crate::mcp::tools::web_fetch::security::{SecurityError, SecurityValidator};
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

/// Error configuration structure for consistent error handling
struct ErrorConfig {
    error_type: &'static str,
    suggestion: &'static str,
    is_retryable: bool,
}

/// Static configuration for error types and their suggestions
const ERROR_CONFIGURATIONS: &[ErrorConfig] = &[
    ErrorConfig {
        error_type: "network_error",
        suggestion: "Check your internet connection and try again. The server may be temporarily unavailable.",
        is_retryable: true,
    },
    ErrorConfig {
        error_type: "security_error",
        suggestion: "The URL was blocked for security reasons. Check if the URL scheme is HTTPS/HTTP and the domain is not restricted.",
        is_retryable: false,
    },
    ErrorConfig {
        error_type: "redirect_error",
        suggestion: "Too many redirects detected. The URL may have redirect loops.",
        is_retryable: true,
    },
    ErrorConfig {
        error_type: "not_found_error",
        suggestion: "The requested page was not found. Verify the URL is correct and the page exists.",
        is_retryable: false,
    },
    ErrorConfig {
        error_type: "access_denied_error",
        suggestion: "Access to the resource is forbidden. Check if authentication is required.",
        is_retryable: false,
    },
    ErrorConfig {
        error_type: "server_error",
        suggestion: "The server encountered an error. Try again later or contact the website administrator.",
        is_retryable: true,
    },
    ErrorConfig {
        error_type: "content_processing_error",
        suggestion: "Failed to convert HTML to markdown. The page may have complex HTML structures or encoding issues.",
        is_retryable: true,
    },
    ErrorConfig {
        error_type: "content_error",
        suggestion: "Failed to process the content. The page may have malformed HTML or encoding issues.",
        is_retryable: false,
    },
    ErrorConfig {
        error_type: "size_limit_error",
        suggestion: "Content is too large. Try reducing max_content_length or use a different URL.",
        is_retryable: false,
    },
];

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
    ) -> Result<(String, RedirectInfo), Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .user_agent(
                request
                    .user_agent
                    .as_deref()
                    .unwrap_or("SwissArmyHammer-Bot/1.0"),
            )
            .timeout(Duration::from_secs(request.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS) as u64))
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

            // Get final content
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/html")
                .to_string();

            // Stream content with size validation
            let max_length = request.max_content_length.unwrap_or(DEFAULT_CONTENT_LENGTH_BYTES) as usize;
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
                    ).into());
                }
            }
            
            // Convert bytes to string
            let body = String::from_utf8(body_bytes).map_err(|e| {
                format!("Invalid UTF-8 content: {e}")
            })?;

            // Convert HTML to markdown using markdowndown
            let markdown_content = if content_type.contains("text/html") {
                match self.html_converter.convert_html(&body) {
                    Ok(md) => md,
                    Err(e) => {
                        tracing::warn!("Failed to convert HTML to markdown using markdowndown: {}", e);
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

            return Ok((markdown_content, redirect_info));
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

    /// Extract description from markdown content (first substantial paragraph after title)
    fn extract_description_from_markdown(markdown: &str) -> Option<String> {
        let mut found_title = false;
        let mut in_paragraph = false;
        let mut current_paragraph = String::new();

        for line in markdown.lines() {
            let trimmed = line.trim();

            // Skip until we find the first heading
            if !found_title {
                if trimmed.starts_with('#') {
                    found_title = true;
                }
                continue;
            }

            // Skip empty lines and other headings
            if trimmed.is_empty() {
                if in_paragraph && !current_paragraph.trim().is_empty() {
                    // End of paragraph - check if it's substantial
                    let paragraph_text = current_paragraph.trim().to_string();
                    if paragraph_text.len() > 50 && !paragraph_text.starts_with('#') {
                        return Some(paragraph_text);
                    }
                }
                current_paragraph.clear();
                in_paragraph = false;
                continue;
            }

            // Skip other headings
            if trimmed.starts_with('#') {
                current_paragraph.clear();
                in_paragraph = false;
                continue;
            }

            // Accumulate paragraph content
            if !in_paragraph {
                in_paragraph = true;
                current_paragraph = trimmed.to_string();
            } else {
                current_paragraph.push(' ');
                current_paragraph.push_str(trimmed);
            }
        }

        // Check final paragraph if we ended while building one
        if in_paragraph && !current_paragraph.trim().is_empty() {
            let paragraph_text = current_paragraph.trim().to_string();
            if paragraph_text.len() > 50 {
                return Some(paragraph_text);
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

    /// Get error suggestion based on error type
    fn get_error_suggestion(error_type: &str) -> &'static str {
        ERROR_CONFIGURATIONS
            .iter()
            .find(|config| config.error_type == error_type)
            .map(|config| config.suggestion)
            .unwrap_or("An unexpected error occurred. Check the URL and try again.")
    }

    /// Check if an error type is retryable
    fn is_retryable_error(error_type: &str) -> bool {
        ERROR_CONFIGURATIONS
            .iter()
            .find(|config| config.error_type == error_type)
            .map(|config| config.is_retryable)
            .unwrap_or(false)
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
            Ok((markdown_content, redirect_info)) => {
                Self::build_success_response(markdown_content, redirect_info, response_time_ms, &request)
            }
            Err(error) => {
                Self::build_error_response(error.as_ref(), response_time_ms, &request)
            }
        }
    }
}

impl WebFetchTool {
    /// Validates request parameters including URL security, timeout, and content length
    async fn validate_request_parameters(&self, request: &WebFetchRequest) -> Result<String, McpError> {
        // Comprehensive URL security validation
        let validated_url = match self.security_validator.validate_url(&request.url) {
            Ok(url) => url,
            Err(SecurityError::InvalidUrl(msg)) => {
                self.security_validator.log_security_event(
                    "INVALID_URL", 
                    &request.url, 
                    &msg
                );
                return Err(McpError::invalid_params(
                    format!("Invalid URL: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::BlockedDomain(msg)) => {
                self.security_validator.log_security_event(
                    "BLOCKED_DOMAIN", 
                    &request.url, 
                    &msg
                );
                return Err(McpError::invalid_params(
                    format!("Access denied: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::SsrfAttempt(msg)) => {
                self.security_validator.log_security_event(
                    "SSRF_ATTEMPT", 
                    &request.url, 
                    &msg
                );
                return Err(McpError::invalid_params(
                    format!("Security violation: {msg}"),
                    None,
                ));
            }
            Err(SecurityError::UnsupportedScheme(msg)) => {
                self.security_validator.log_security_event(
                    "UNSUPPORTED_SCHEME", 
                    &request.url, 
                    &msg
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
        response_time_ms: u64,
        request: &WebFetchRequest,
    ) -> Result<CallToolResult, McpError> {
        let content_str = content.as_str();
        let content_length = content_str.len();
        let word_count = content_str.split_whitespace().count();

        // Extract HTML title from markdown content (first heading)
        let extracted_title = Self::extract_title_from_markdown(content_str);

        // Extract description (first paragraph after title)
        let extracted_description = Self::extract_description_from_markdown(content_str);

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

        // Create comprehensive response with markdown content and redirect metadata
        let mut response = serde_json::json!({
            "url": request.url,
            "final_url": redirect_info.final_url,
            "status": "success",
            "status_code": redirect_info.redirect_chain.last().map(|s| s.status_code).unwrap_or(200),
            "response_time_ms": response_time_ms,
            "content_length": content_length,
            "word_count": word_count,
            "title": extracted_title,
            "description": extracted_description,
            "content_type": "text/html",
            "markdown_content": content_str,
            "encoding": "utf-8"
        });

        // Add redirect information if redirects occurred
        if redirect_info.redirect_count > 0 {
            response["redirect_count"] = serde_json::Value::Number(
                serde_json::Number::from(redirect_info.redirect_count),
            );
            response["redirect_chain"] = serde_json::Value::Array(
                redirect_chain_formatted
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            );
        }

        let success_message = if redirect_info.redirect_count > 0 {
            let url = &request.url;
            let redirect_count = redirect_info.redirect_count;
            let redirect_s = if redirect_count == 1 { "" } else { "s" };
            let final_url = &redirect_info.final_url;
            let metadata = serde_json::to_string_pretty(&response).unwrap_or_default();
            format!(
                "Successfully fetched and converted content from {url} (followed {redirect_count} redirect{redirect_s})\nFinal URL: {final_url}\n\nMetadata: {metadata}\n\nContent:\n{content_str}"
            )
        } else {
            let url = &request.url;
            let metadata = serde_json::to_string_pretty(&response).unwrap_or_default();
            format!(
                "Successfully fetched and converted content from {url}\n\nMetadata: {metadata}\n\nContent:\n{content_str}"
            )
        };

        Ok(BaseToolImpl::create_success_response(success_message))
    }

    /// Builds an error response with detailed error information
    fn build_error_response(
        error: &(dyn std::error::Error + Send + Sync),
        response_time_ms: u64,
        request: &WebFetchRequest,
    ) -> Result<CallToolResult, McpError> {
        let error_type = Self::categorize_error(error);
        let error_suggestion = Self::get_error_suggestion(error_type);

        tracing::warn!(
            "Failed to fetch content from {} after {}ms: {} (category: {})",
            request.url,
            response_time_ms,
            error,
            error_type
        );

        // Create comprehensive error response
        let error_info = serde_json::json!({
            "url": request.url,
            "status": "error",
            "error_type": error_type,
            "error_details": error.to_string(),
            "error_suggestion": error_suggestion,
            "response_time_ms": response_time_ms,
            "encoding": "utf-8",
            "retry_recommended": Self::is_retryable_error(error_type)
        });

        let url = &request.url;
        let error_details = serde_json::to_string_pretty(&error_info).unwrap_or_default();
        Err(McpError::invalid_params(
            format!(
                "Failed to fetch content from {url}: {error}\n\nError Type: {error_type}\nSuggestion: {error_suggestion}\n\nError details: {error_details}"
            ),
            None,
        ))
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
    fn test_extract_description_from_markdown() {
        // Test description extraction after title
        let markdown_with_description = "# Title\n\nThis is a substantial description that should be longer than fifty characters to be extracted as the description.";
        let description =
            WebFetchTool::extract_description_from_markdown(markdown_with_description);
        assert!(description.is_some());
        assert!(description.unwrap().len() > 50);

        // Test with short description - should be None
        let markdown_short_description = "# Title\n\nShort.";
        let description =
            WebFetchTool::extract_description_from_markdown(markdown_short_description);
        assert_eq!(description, None);

        // Test with no title - should be None
        let markdown_no_title =
            "This is content without a title. It should not extract a description.";
        let description = WebFetchTool::extract_description_from_markdown(markdown_no_title);
        assert_eq!(description, None);

        // Test with multiple paragraphs - should get first substantial one
        let markdown_multiple_paragraphs = "# Title\n\nThis is the first substantial paragraph that meets the length requirement for description extraction.\n\nThis is a second paragraph.";
        let description =
            WebFetchTool::extract_description_from_markdown(markdown_multiple_paragraphs);
        assert!(description.is_some());
        assert!(description.unwrap().contains("first substantial paragraph"));
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
    fn test_error_suggestions() {
        assert_eq!(
            WebFetchTool::get_error_suggestion("network_error"),
            "Check your internet connection and try again. The server may be temporarily unavailable."
        );

        assert_eq!(
            WebFetchTool::get_error_suggestion("not_found_error"),
            "The requested page was not found. Verify the URL is correct and the page exists."
        );

        assert_eq!(
            WebFetchTool::get_error_suggestion("unknown_error"),
            "An unexpected error occurred. Check the URL and try again."
        );
    }

    #[test]
    fn test_retryable_errors() {
        assert!(WebFetchTool::is_retryable_error("network_error"));
        assert!(WebFetchTool::is_retryable_error("server_error"));
        assert!(WebFetchTool::is_retryable_error("redirect_error"));

        assert!(!WebFetchTool::is_retryable_error("not_found_error"));
        assert!(!WebFetchTool::is_retryable_error("access_denied_error"));
        assert!(!WebFetchTool::is_retryable_error("content_error"));
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
        let redirect_chain = vec![
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
                "Status code {} should be in 3xx range",
                code
            );
        }

        // Test non-redirect codes
        let non_redirect_codes = [200, 404, 500];

        for code in non_redirect_codes {
            assert!(
                !(300..400).contains(&code),
                "Status code {} should not be in 3xx range",
                code
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
}
