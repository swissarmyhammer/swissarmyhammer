//! Web fetch pipeline — internal implementation for URL fetching
//!
//! This module provides the `WebFetchTool` struct with reusable fetch pipeline methods.
//! Registration is handled by the unified `web` tool module (`tools::web`).

use crate::mcp::tools::web_fetch::security::{SecurityError, SecurityValidator};
use crate::mcp::types::WebFetchRequest;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

/// Configuration constants for web fetch operations
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;

/// Reusable web fetch pipeline providing URL validation, content fetching, and response building.
///
/// This struct is used internally by the unified `web` tool and is not registered as
/// a standalone MCP tool.
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
    pub async fn validate_request_parameters(
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
    pub fn create_markdowndown_config(&self, request: &WebFetchRequest) -> markdowndown::Config {
        let mut config = markdowndown::Config::default();

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

        config
    }

    /// Builds a success response that returns only the fetched content
    pub fn build_success_response(
        &self,
        _request: &WebFetchRequest,
        markdown_content: String,
        _response_time_ms: u64,
    ) -> Result<CallToolResult, McpError> {
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
    pub fn build_error_response(
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

        let metadata = serde_json::json!({
            "url": request.url,
            "error_type": error_type,
            "error_details": error.to_string(),
            "status_code": null,
            "response_time_ms": response_time_ms,
            "performance_impact": if response_time_ms > 10000 { "high" } else { "low" },
            "optimization_enabled": true
        });

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::WebFetchRequest;

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

        let ssl_error = std::io::Error::other("SSL certificate error");
        assert_eq!(tool.categorize_error(&ssl_error), "ssl_error");

        let parse_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid encoding");
        assert_eq!(tool.categorize_error(&parse_error), "content_error");
    }
}
