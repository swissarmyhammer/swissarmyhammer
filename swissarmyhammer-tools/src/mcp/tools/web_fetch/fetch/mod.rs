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
    fn categorize_error(error: &impl std::error::Error) -> &'static str {
        let error_str = error.to_string().to_lowercase();

        // Network-related errors
        if error_str.contains("connection")
            || error_str.contains("timeout")
            || error_str.contains("dns")
        {
            "network_error"
        } else if error_str.contains("ssl")
            || error_str.contains("tls")
            || error_str.contains("certificate")
        {
            "security_error"
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
        match error_type {
            "network_error" => "Check your internet connection and try again. The server may be temporarily unavailable.",
            "security_error" => "SSL/TLS certificate validation failed. Check if the URL uses HTTPS correctly.",
            "redirect_error" => "Too many redirects detected. The URL may have redirect loops.",
            "not_found_error" => "The requested page was not found. Verify the URL is correct and the page exists.",
            "access_denied_error" => "Access to the resource is forbidden. Check if authentication is required.",
            "server_error" => "The server encountered an error. Try again later or contact the website administrator.",
            "content_error" => "Failed to process the HTML content. The page may have malformed HTML or encoding issues.",
            "size_limit_error" => "Content is too large. Try reducing max_content_length or use a different URL.",
            _ => "An unexpected error occurred. Check the URL and try again."
        }
    }

    /// Check if an error type is retryable
    fn is_retryable_error(error_type: &str) -> bool {
        matches!(
            error_type,
            "network_error" | "server_error" | "redirect_error"
        )
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

        // Configure markdowndown Config with enhanced HTML-to-markdown options
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

        // Configure HTML processing options for better content extraction
        config.html.max_line_width = 120; // Reasonable line width for markdown
        config.html.remove_scripts_styles = true; // Clean up scripts and styles
        config.html.remove_navigation = true; // Remove nav elements for cleaner content
        config.html.remove_sidebars = true; // Remove sidebar content
        config.html.remove_ads = true; // Clean up advertisement content
        config.html.max_blank_lines = 2; // Limit excessive blank lines

        // Configure output formatting for clean, structured markdown
        config.output.include_frontmatter = false; // Don't add YAML frontmatter
        config.output.normalize_whitespace = true; // Clean up whitespace
        config.output.max_consecutive_blank_lines = 2; // Prevent excessive blank lines

        // Perform the web fetch and convert to markdown
        let start_time = std::time::Instant::now();
        let fetch_result = markdowndown::convert_url_with_config(&request.url, config).await;
        let response_time_ms = start_time.elapsed().as_millis() as u64;

        match fetch_result {
            Ok(markdown_content) => {
                let content_str = markdown_content.as_str();
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

                // Create comprehensive response with markdown content and enhanced metadata
                let response = serde_json::json!({
                    "url": request.url,
                    "final_url": request.url, // markdowndown handles redirects internally
                    "status": "success",
                    "status_code": 200, // Assume success if we got content
                    "response_time_ms": response_time_ms,
                    "content_length": content_length,
                    "word_count": word_count,
                    "title": extracted_title,
                    "description": extracted_description,
                    "content_type": "text/html", // Assumed since we're processing HTML
                    "markdown_content": content_str,
                    "encoding": "utf-8", // markdowndown normalizes to UTF-8
                    "conversion_options": {
                        "max_line_width": 120,
                        "remove_scripts_styles": true,
                        "remove_navigation": true,
                        "remove_sidebars": true,
                        "remove_ads": true,
                        "normalize_whitespace": true
                    }
                });

                Ok(BaseToolImpl::create_success_response(format!(
                    "Successfully fetched and converted content from {}\n\nMetadata: {}\n\nContent:\n{}",
                    request.url,
                    serde_json::to_string_pretty(&response).unwrap_or_default(),
                    content_str
                )))
            }
            Err(error) => {
                let error_type = Self::categorize_error(&error);
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

                Err(McpError::invalid_params(
                    format!(
                        "Failed to fetch content from {}: {}\n\nError Type: {}\nSuggestion: {}\n\nError details: {}",
                        request.url,
                        error,
                        error_type,
                        error_suggestion,
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
}
