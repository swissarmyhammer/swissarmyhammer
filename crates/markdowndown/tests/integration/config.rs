//! Integration test configuration module
//!
//! Provides configuration management for integration tests with external services.

use std::env;
use std::time::Duration;

// Default rate limiting and timeout constants
const DEFAULT_REQUESTS_PER_MINUTE: u32 = 30;
const DEFAULT_REQUEST_DELAY_MS: u64 = 2000;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_LARGE_DOCUMENT_TIMEOUT_SECS: u64 = 120;

// Local testing configuration constants
const LOCAL_TESTING_REQUESTS_PER_MINUTE: u32 = 10;
const LOCAL_TESTING_REQUEST_DELAY_MS: u64 = 6000;

// CI configuration constants
const CI_REQUESTS_PER_MINUTE: u32 = 60;
const CI_REQUEST_DELAY_MS: u64 = 1000;
const CI_DEFAULT_TIMEOUT_SECS: u64 = 60;
const CI_LARGE_DOCUMENT_TIMEOUT_SECS: u64 = 180;

// Validation thresholds
const MIN_VALID_CONTENT_LENGTH: usize = 50;
const MIN_VALID_LINE_COUNT: usize = 3;

// Concurrent test configuration
const DEFAULT_CONCURRENT_TEST_COUNT: usize = 2;

/// Configuration for integration tests with external services
#[derive(Debug, Clone)]
pub struct IntegrationTestConfig {
    // Rate limiting
    pub requests_per_minute: u32,
    pub request_delay_ms: u64,

    // Timeouts
    pub default_timeout_secs: u64,
    pub large_document_timeout_secs: u64,

    // Authentication
    pub github_token: Option<String>,
    pub google_api_key: Option<String>,

    // Test control
    pub skip_slow_tests: bool,
    pub skip_external_services: bool,
    pub skip_network_tests: bool,

    // Test concurrency
    pub concurrent_test_count: usize,
}

/// Parse a u32 value from an environment variable with a default fallback
fn parse_env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Parse a u64 value from an environment variable with a default fallback
fn parse_env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Parse a boolean value from an environment variable with a default fallback
fn parse_env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|s| matches!(s.as_str(), "true" | "True" | "TRUE" | "1"))
        .unwrap_or(default)
}

impl IntegrationTestConfig {
    /// Create a base configuration with rate limiting and timeout settings
    fn with_rates(
        requests_per_minute: u32,
        request_delay_ms: u64,
        default_timeout_secs: u64,
        large_document_timeout_secs: u64,
    ) -> Self {
        Self {
            requests_per_minute,
            request_delay_ms,
            default_timeout_secs,
            large_document_timeout_secs,
            github_token: env::var("GITHUB_TOKEN").ok(),
            google_api_key: env::var("GOOGLE_API_KEY").ok(),
            skip_slow_tests: false,
            skip_external_services: false,
            skip_network_tests: false,
            concurrent_test_count: DEFAULT_CONCURRENT_TEST_COUNT,
        }
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            requests_per_minute: parse_env_u32(
                "INTEGRATION_REQUESTS_PER_MINUTE",
                DEFAULT_REQUESTS_PER_MINUTE,
            ),
            request_delay_ms: parse_env_u64(
                "INTEGRATION_REQUEST_DELAY_MS",
                DEFAULT_REQUEST_DELAY_MS,
            ),
            default_timeout_secs: parse_env_u64(
                "INTEGRATION_DEFAULT_TIMEOUT_SECS",
                DEFAULT_TIMEOUT_SECS,
            ),
            large_document_timeout_secs: parse_env_u64(
                "INTEGRATION_LARGE_TIMEOUT_SECS",
                DEFAULT_LARGE_DOCUMENT_TIMEOUT_SECS,
            ),
            github_token: env::var("GITHUB_TOKEN").ok(),
            google_api_key: env::var("GOOGLE_API_KEY").ok(),
            skip_slow_tests: parse_env_bool("SKIP_SLOW_TESTS", false),
            skip_external_services: parse_env_bool("SKIP_EXTERNAL_SERVICES", false),
            skip_network_tests: parse_env_bool("SKIP_NETWORK_TESTS", false),
            concurrent_test_count: parse_env_u32(
                "INTEGRATION_CONCURRENT_TEST_COUNT",
                DEFAULT_CONCURRENT_TEST_COUNT as u32,
            ) as usize,
        }
    }

    /// Create a test configuration with defaults for local testing
    pub fn for_local_testing() -> Self {
        Self::with_rates(
            LOCAL_TESTING_REQUESTS_PER_MINUTE,
            LOCAL_TESTING_REQUEST_DELAY_MS,
            DEFAULT_TIMEOUT_SECS,
            DEFAULT_LARGE_DOCUMENT_TIMEOUT_SECS,
        )
    }

    /// Create a CI-friendly configuration that skips tests requiring credentials
    pub fn for_ci() -> Self {
        let mut config = Self::with_rates(
            CI_REQUESTS_PER_MINUTE,
            CI_REQUEST_DELAY_MS,
            CI_DEFAULT_TIMEOUT_SECS,
            CI_LARGE_DOCUMENT_TIMEOUT_SECS,
        );
        config.skip_slow_tests = parse_env_bool("SKIP_SLOW_TESTS", true);
        config.skip_external_services = parse_env_bool("SKIP_EXTERNAL_SERVICES", false);
        config
    }

    /// Get the delay duration between requests
    pub fn request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }

    /// Get the default timeout duration
    pub fn default_timeout(&self) -> Duration {
        Duration::from_secs(self.default_timeout_secs)
    }

    /// Get the large document timeout duration
    pub fn large_document_timeout(&self) -> Duration {
        Duration::from_secs(self.large_document_timeout_secs)
    }

    /// Check if GitHub tests can be run (token available)
    pub fn can_test_github(&self) -> bool {
        !self.skip_external_services && self.github_token.is_some()
    }

    /// Check if Google Docs tests can be run
    pub fn can_test_google_docs(&self) -> bool {
        !self.skip_external_services
    }

    /// Check if HTML tests can be run
    pub fn can_test_html(&self) -> bool {
        !self.skip_external_services && !self.skip_network_tests
    }
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Test URL type for categorizing different HTML test scenarios
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestUrlType {
    /// Simple HTML page for basic testing
    Simple,
    /// Complex HTML with rich content (Wikipedia-like)
    Complex,
    /// Technical documentation
    Documentation,
    /// Source code hosting (GitHub-like)
    SourceCode,
}

/// Test URL collections for different services
pub struct TestUrls;

impl TestUrls {
    /// Stable HTML test URLs that should remain accessible
    /// Format: (url, description, type)
    pub const HTML_TEST_URLS: &'static [(&'static str, &'static str, TestUrlType)] = &[
        ("https://httpbin.org/html", "Simple HTML test page", TestUrlType::Simple),
        (
            "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            "Complex Wikipedia page",
            TestUrlType::Complex,
        ),
        (
            "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
            "Rust book chapter",
            TestUrlType::Documentation,
        ),
        (
            "https://github.com/rust-lang/rust/blob/master/README.md",
            "GitHub README",
            TestUrlType::SourceCode,
        ),
    ];

    /// Error test URLs that provide predictable error responses
    /// Format: (url, description, expected_error_indicator)
    pub const ERROR_TEST_URLS: &'static [(&'static str, &'static str, &'static str)] = &[
        ("https://httpbin.org/status/404", "HTTP 404 error", "404"),
        ("https://httpbin.org/status/500", "HTTP 500 error", "500"),
        (
            "https://invalid-domain-that-should-not-exist-12345.example/page",
            "DNS resolution failure",
            "dns",
        ),
    ];

    /// Get a test URL of a specific type, if available
    pub fn get_url_by_type(url_type: TestUrlType) -> Option<(&'static str, &'static str)> {
        Self::HTML_TEST_URLS
            .iter()
            .find(|(_, _, t)| *t == url_type)
            .map(|(url, desc, _)| (*url, *desc))
    }

    /// GitHub test URLs for issues and pull requests
    pub const GITHUB_TEST_URLS: &'static [(&'static str, &'static str)] = &[
        (
            "https://github.com/rust-lang/rust/issues/1",
            "Historic issue #1",
        ),
        (
            "https://github.com/tokio-rs/tokio/issues/1000",
            "Issue with discussions",
        ),
        (
            "https://github.com/serde-rs/serde/pull/2000",
            "Pull request example",
        ),
    ];
}

/// Utility functions for integration tests
pub struct TestUtils;

impl TestUtils {
    /// Apply rate limiting delay if configured
    pub async fn apply_rate_limit(config: &IntegrationTestConfig) {
        if config.request_delay_ms > 0 {
            tokio::time::sleep(config.request_delay()).await;
        }
    }

    /// Check if content looks like valid markdown
    ///
    /// # Arguments
    ///
    /// * `content` - The markdown content to validate
    /// * `min_length` - Minimum content length (default: 50)
    /// * `min_lines` - Minimum line count (default: 3)
    pub fn validate_markdown_quality(content: &str) -> bool {
        Self::validate_markdown_quality_with_thresholds(
            content,
            MIN_VALID_CONTENT_LENGTH,
            MIN_VALID_LINE_COUNT,
        )
    }

    /// Check if content looks like valid markdown with configurable thresholds
    ///
    /// # Arguments
    ///
    /// * `content` - The markdown content to validate
    /// * `min_length` - Minimum content length
    /// * `min_lines` - Minimum line count
    pub fn validate_markdown_quality_with_thresholds(
        content: &str,
        min_length: usize,
        min_lines: usize,
    ) -> bool {
        // Basic quality checks
        !content.is_empty()
            && content.len() > min_length
            && !content.trim().starts_with("Error") // Should not be an error message
            && content.lines().count() > min_lines
    }

    /// Validate that frontmatter contains expected fields
    pub fn validate_frontmatter(frontmatter: &str) -> bool {
        frontmatter.contains("source_url")
            && frontmatter.contains("converted_at")
            && frontmatter.contains("conversion_type")
    }

    /// Get a user agent string for testing
    pub fn test_user_agent() -> String {
        format!(
            "markdowndown-integration-tests/{}",
            env!("CARGO_PKG_VERSION")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_with_defaults() {
        // Test that config creation works even without environment variables
        let config = IntegrationTestConfig::from_env();

        assert_eq!(config.requests_per_minute, DEFAULT_REQUESTS_PER_MINUTE);
        assert_eq!(config.request_delay_ms, DEFAULT_REQUEST_DELAY_MS);
        assert_eq!(config.default_timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(
            config.large_document_timeout_secs,
            DEFAULT_LARGE_DOCUMENT_TIMEOUT_SECS
        );
        assert!(!config.skip_slow_tests || env::var("SKIP_SLOW_TESTS").is_ok());
    }

    #[test]
    fn test_local_testing_config() {
        let config = IntegrationTestConfig::for_local_testing();

        assert_eq!(
            config.requests_per_minute,
            LOCAL_TESTING_REQUESTS_PER_MINUTE
        );
        assert_eq!(config.request_delay_ms, LOCAL_TESTING_REQUEST_DELAY_MS);
        assert!(!config.skip_slow_tests);
        assert!(!config.skip_external_services);
    }

    #[test]
    fn test_ci_config() {
        let config = IntegrationTestConfig::for_ci();

        assert_eq!(config.requests_per_minute, CI_REQUESTS_PER_MINUTE);
        assert_eq!(config.request_delay_ms, CI_REQUEST_DELAY_MS);
        // CI should skip slow tests by default unless overridden
        assert!(
            config.skip_slow_tests
                || env::var("SKIP_SLOW_TESTS")
                    .map(|s| matches!(s.as_str(), "false" | "False" | "FALSE" | "0"))
                    .unwrap_or(false)
        );
    }

    #[test]
    fn test_duration_helpers() {
        let config = IntegrationTestConfig::for_local_testing();

        assert_eq!(
            config.request_delay(),
            Duration::from_millis(LOCAL_TESTING_REQUEST_DELAY_MS)
        );
        assert_eq!(
            config.default_timeout(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS)
        );
        assert_eq!(
            config.large_document_timeout(),
            Duration::from_secs(DEFAULT_LARGE_DOCUMENT_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_capability_checks() {
        let config = IntegrationTestConfig::for_local_testing();

        // These depend on environment variables, so we just test the logic
        assert_eq!(config.can_test_github(), config.github_token.is_some());
        assert!(config.can_test_google_docs()); // Should be true for local testing
        assert!(config.can_test_html()); // Should be true for local testing
    }

    #[test]
    fn test_validation_helpers() {
        // Test markdown quality validation
        assert!(TestUtils::validate_markdown_quality(
            "# Title\n\nThis is a substantial piece of content that should pass validation.\n\nIt has multiple lines and good content."
        ));
        assert!(!TestUtils::validate_markdown_quality(""));
        assert!(!TestUtils::validate_markdown_quality("Short"));
        assert!(!TestUtils::validate_markdown_quality(
            "Error: Something went wrong"
        ));

        // Test frontmatter validation
        assert!(TestUtils::validate_frontmatter(
            "source_url: test\nconverted_at: now\nconversion_type: html"
        ));
        assert!(!TestUtils::validate_frontmatter("missing_fields: true"));
    }

    #[test]
    fn test_user_agent() {
        let ua = TestUtils::test_user_agent();
        assert!(ua.starts_with("markdowndown-integration-tests/"));
        assert!(ua.contains(env!("CARGO_PKG_VERSION")));
    }
}
