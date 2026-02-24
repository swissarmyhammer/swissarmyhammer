//! Web fetch pipeline — URL fetching with HTML-to-markdown conversion
//!
//! This module provides the `WebFetcher` struct with reusable fetch pipeline methods.
//! It handles URL validation, security checks, content fetching, and error categorization.

use crate::security::{SecurityError, SecurityValidator};
use crate::types::WebFetchRequest;

/// Configuration constants for web fetch operations
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;

/// Result of a successful URL fetch
#[derive(Debug)]
pub struct FetchResult {
    /// The fetched content converted to markdown
    pub markdown: String,
    /// Time taken for the fetch operation in milliseconds
    pub response_time_ms: u64,
}

/// Reusable web fetch pipeline providing URL validation, content fetching, and error categorization.
pub struct WebFetcher {
    /// Security validator for URL and domain validation
    security_validator: SecurityValidator,
}

impl Default for WebFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetcher {
    /// Creates a new instance of the WebFetcher
    pub fn new() -> Self {
        Self {
            security_validator: SecurityValidator::new(),
        }
    }

    /// Validates request parameters including URL security and parameter ranges.
    /// Returns the validated URL string on success.
    pub async fn validate_url(&self, request: &WebFetchRequest) -> Result<String, FetchError> {
        match self.security_validator.validate_url(&request.url) {
            Ok(url) => Ok(url.to_string()),
            Err(SecurityError::InvalidUrl(msg)) => {
                self.security_validator
                    .log_security_event("INVALID_URL", &request.url, &msg);
                Err(FetchError::InvalidUrl(format!("Invalid URL: {msg}")))
            }
            Err(SecurityError::UnsupportedScheme(protocol)) => {
                self.security_validator.log_security_event(
                    "UNSUPPORTED_PROTOCOL",
                    &request.url,
                    &format!("Protocol: {protocol}"),
                );
                Err(FetchError::InvalidUrl(format!(
                    "Unsupported protocol: {protocol}. Only HTTP and HTTPS are supported."
                )))
            }
            Err(SecurityError::SsrfAttempt(network)) => {
                self.security_validator.log_security_event(
                    "PRIVATE_NETWORK_ACCESS_ATTEMPT",
                    &request.url,
                    &format!("Network: {network}"),
                );
                Err(FetchError::SecurityViolation(format!(
                    "Access to private network not allowed: {network}"
                )))
            }
            Err(e) => {
                self.security_validator.log_security_event(
                    "SECURITY_VALIDATION_FAILED",
                    &request.url,
                    &e.to_string(),
                );
                Err(FetchError::SecurityViolation(format!(
                    "Security validation failed: {e}"
                )))
            }
        }
    }

    /// Converts WebFetchRequest parameters to markdowndown Config
    pub fn create_markdowndown_config(&self, request: &WebFetchRequest) -> markdowndown::Config {
        let mut config = markdowndown::Config::default();

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

    /// Fetch a URL and return the content as markdown.
    pub async fn fetch_url(&self, request: &WebFetchRequest) -> Result<FetchResult, FetchError> {
        let validated_url = self.validate_url(request).await?;
        let config = self.create_markdowndown_config(request);

        let start_time = std::time::Instant::now();

        match markdowndown::convert_url_with_config(&validated_url, config).await {
            Ok(markdown) => Ok(FetchResult {
                markdown: markdown.to_string(),
                response_time_ms: start_time.elapsed().as_millis() as u64,
            }),
            Err(e) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                tracing::warn!(
                    "Failed to fetch content from {} after {}ms: {}",
                    request.url,
                    response_time_ms,
                    e
                );
                Err(FetchError::FetchFailed {
                    error_type: Self::categorize_error(&e),
                    message: e.to_string(),
                    response_time_ms,
                })
            }
        }
    }

    /// Categorizes errors for proper error handling and response formatting
    pub fn categorize_error(error: &dyn std::error::Error) -> String {
        let error_str = error.to_string().to_lowercase();

        if error_str.contains("timeout") || error_str.contains("timed out") {
            "timeout".to_string()
        } else if error_str.contains("connection")
            || error_str.contains("network")
            || error_str.contains("dns")
            || error_str.contains("resolve")
        {
            "network_error".to_string()
        } else if error_str.contains("ssl")
            || error_str.contains("tls")
            || error_str.contains("certificate")
        {
            "ssl_error".to_string()
        } else if error_str.contains("redirect") {
            "redirect_error".to_string()
        } else if error_str.contains("401") || error_str.contains("403") {
            "auth_error".to_string()
        } else if error_str.contains("404") {
            "not_found".to_string()
        } else if error_str.contains("400") {
            "client_error".to_string()
        } else if error_str.contains("500")
            || error_str.contains("502")
            || error_str.contains("503")
        {
            "server_error".to_string()
        } else if error_str.contains("parse")
            || error_str.contains("encoding")
            || error_str.contains("invalid")
        {
            "content_error".to_string()
        } else if error_str.contains("too large") || error_str.contains("size") {
            "size_limit_error".to_string()
        } else {
            "unknown_error".to_string()
        }
    }
}

/// Errors that can occur during web fetch operations
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// URL is invalid or malformed
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Security policy violation (SSRF, blocked domain, etc.)
    #[error("Security violation: {0}")]
    SecurityViolation(String),

    /// Fetch operation failed
    #[error("Fetch failed ({error_type}): {message}")]
    FetchFailed {
        /// Error category
        error_type: String,
        /// Detailed error message
        message: String,
        /// Time taken before failure in milliseconds
        response_time_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WebFetchRequest;

    #[test]
    fn test_create_markdowndown_config() {
        let fetcher = WebFetcher::new();

        // Test with default values
        let request = WebFetchRequest {
            url: "https://example.com".to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };

        let config = fetcher.create_markdowndown_config(&request);
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

        let config = fetcher.create_markdowndown_config(&request);
        assert_eq!(config.http.timeout, std::time::Duration::from_secs(60));
        assert_eq!(config.http.user_agent, "CustomAgent/1.0");
        assert_eq!(config.http.max_redirects, 0);
    }

    #[test]
    fn test_categorize_error() {
        let timeout_error =
            std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timed out");
        assert_eq!(WebFetcher::categorize_error(&timeout_error), "timeout");

        let network_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        assert_eq!(
            WebFetcher::categorize_error(&network_error),
            "network_error"
        );

        let ssl_error = std::io::Error::other("SSL certificate error");
        assert_eq!(WebFetcher::categorize_error(&ssl_error), "ssl_error");

        let parse_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid encoding");
        assert_eq!(WebFetcher::categorize_error(&parse_error), "content_error");
    }
}
