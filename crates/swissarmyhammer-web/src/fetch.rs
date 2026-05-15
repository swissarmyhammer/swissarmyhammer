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

    /// Helper to build a minimal WebFetchRequest for a given URL.
    fn request(url: &str) -> WebFetchRequest {
        WebFetchRequest {
            url: url.to_string(),
            timeout: None,
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        }
    }

    #[tokio::test]
    async fn test_validate_url_valid() {
        let fetcher = WebFetcher::new();
        let result = fetcher.validate_url(&request("https://example.com")).await;
        assert!(result.is_ok(), "Expected Ok for valid URL, got: {result:?}");
        assert_eq!(result.unwrap(), "https://example.com/");
    }

    #[tokio::test]
    async fn test_validate_url_invalid() {
        let fetcher = WebFetcher::new();
        let result = fetcher.validate_url(&request("not-a-valid-url")).await;
        assert!(
            matches!(result, Err(FetchError::InvalidUrl(_))),
            "Expected InvalidUrl for malformed URL, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_validate_url_unsupported_scheme() {
        let fetcher = WebFetcher::new();
        let result = fetcher
            .validate_url(&request("ftp://example.com/file"))
            .await;
        match &result {
            Err(FetchError::InvalidUrl(msg)) => {
                assert!(
                    msg.contains("protocol") || msg.contains("Unsupported"),
                    "Error should mention unsupported protocol, got: {msg}"
                );
            }
            other => panic!("Expected InvalidUrl for ftp scheme, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_validate_url_ssrf_loopback() {
        let fetcher = WebFetcher::new();
        let result = fetcher
            .validate_url(&request("http://127.0.0.1/path"))
            .await;
        assert!(
            matches!(result, Err(FetchError::SecurityViolation(_))),
            "Expected SecurityViolation for loopback SSRF, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_validate_url_blocked_domain() {
        let fetcher = WebFetcher::new();
        // "metadata.google.internal" is on the default blocked-domains list
        // and also matches the ".internal" blocked pattern — both map to
        // SecurityError::BlockedDomain which falls through to SecurityViolation.
        let result = fetcher
            .validate_url(&request("https://metadata.google.internal/computeMetadata"))
            .await;
        assert!(
            matches!(result, Err(FetchError::SecurityViolation(_))),
            "Expected SecurityViolation for blocked domain, got: {result:?}"
        );
    }

    #[test]
    fn test_categorize_error_timeout() {
        let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timed out");
        assert_eq!(WebFetcher::categorize_error(&err), "timeout");

        let err = std::io::Error::other("request timeout exceeded");
        assert_eq!(WebFetcher::categorize_error(&err), "timeout");
    }

    #[test]
    fn test_categorize_error_network() {
        let err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        assert_eq!(WebFetcher::categorize_error(&err), "network_error");

        let err = std::io::Error::other("network unreachable");
        assert_eq!(WebFetcher::categorize_error(&err), "network_error");

        let err = std::io::Error::other("dns lookup failed");
        assert_eq!(WebFetcher::categorize_error(&err), "network_error");

        let err = std::io::Error::other("could not resolve host");
        assert_eq!(WebFetcher::categorize_error(&err), "network_error");
    }

    #[test]
    fn test_categorize_error_ssl() {
        let err = std::io::Error::other("SSL certificate error");
        assert_eq!(WebFetcher::categorize_error(&err), "ssl_error");

        let err = std::io::Error::other("TLS handshake failed");
        assert_eq!(WebFetcher::categorize_error(&err), "ssl_error");

        let err = std::io::Error::other("certificate verify failed");
        assert_eq!(WebFetcher::categorize_error(&err), "ssl_error");
    }

    #[test]
    fn test_categorize_error_redirect() {
        let err = std::io::Error::other("too many redirect hops");
        assert_eq!(WebFetcher::categorize_error(&err), "redirect_error");
    }

    #[test]
    fn test_categorize_error_auth() {
        let err = std::io::Error::other("server returned 401 unauthorized");
        assert_eq!(WebFetcher::categorize_error(&err), "auth_error");

        let err = std::io::Error::other("server returned 403 forbidden");
        assert_eq!(WebFetcher::categorize_error(&err), "auth_error");
    }

    #[test]
    fn test_categorize_error_not_found() {
        let err = std::io::Error::other("server returned 404 not found");
        assert_eq!(WebFetcher::categorize_error(&err), "not_found");
    }

    #[test]
    fn test_categorize_error_client_error() {
        let err = std::io::Error::other("server returned 400 bad request");
        assert_eq!(WebFetcher::categorize_error(&err), "client_error");
    }

    #[test]
    fn test_categorize_error_server_error() {
        let err = std::io::Error::other("server returned 500 internal server error");
        assert_eq!(WebFetcher::categorize_error(&err), "server_error");

        let err = std::io::Error::other("502 bad gateway");
        assert_eq!(WebFetcher::categorize_error(&err), "server_error");

        let err = std::io::Error::other("503 service unavailable");
        assert_eq!(WebFetcher::categorize_error(&err), "server_error");
    }

    #[test]
    fn test_categorize_error_content_error() {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid encoding");
        assert_eq!(WebFetcher::categorize_error(&err), "content_error");

        let err = std::io::Error::other("failed to parse HTML");
        assert_eq!(WebFetcher::categorize_error(&err), "content_error");
    }

    #[test]
    fn test_categorize_error_size_limit() {
        let err = std::io::Error::other("response body too large");
        assert_eq!(WebFetcher::categorize_error(&err), "size_limit_error");

        let err = std::io::Error::other("content size exceeded limit");
        assert_eq!(WebFetcher::categorize_error(&err), "size_limit_error");
    }

    #[test]
    fn test_categorize_error_unknown() {
        let err = std::io::Error::other("something completely unexpected happened");
        assert_eq!(WebFetcher::categorize_error(&err), "unknown_error");
    }

    #[tokio::test]
    async fn test_fetch_url_failure_path() {
        // Use a URL that will pass validation but fail on actual fetch
        // (non-routable address with a short timeout to fail quickly)
        let fetcher = WebFetcher::new();
        let req = WebFetchRequest {
            url: "https://192.0.2.1/nonexistent".to_string(), // TEST-NET-1, non-routable
            timeout: Some(1),
            follow_redirects: None,
            max_content_length: None,
            user_agent: None,
        };
        // 192.0.2.1 is in a reserved test range that might be blocked by SSRF checks.
        // If validation fails, try a domain that resolves but refuses connections.
        let result = fetcher.fetch_url(&req).await;
        assert!(
            result.is_err(),
            "Expected fetch to fail for unreachable host"
        );
        match result.unwrap_err() {
            FetchError::FetchFailed {
                error_type,
                message,
                response_time_ms: _,
            } => {
                // The error_type should be one of the categorized strings
                assert!(
                    !error_type.is_empty(),
                    "error_type should not be empty, got message: {message}"
                );
            }
            FetchError::SecurityViolation(_) => {
                // Also acceptable — the SSRF guard may block this IP range
            }
            other => panic!("Expected FetchFailed or SecurityViolation, got: {other:?}"),
        }
    }
}
