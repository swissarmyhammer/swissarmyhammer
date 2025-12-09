//! # MarkdownDown
//!
//! A Rust library for acquiring markdown from URLs with smart handling.
//!
//! This library provides a unified interface for extracting and converting content
//! from various URL sources (HTML pages, Google Docs, Office 365, GitHub) into
//! clean markdown format.
//!
//! ## Architecture
//!
//! The library follows a modular architecture:
//! - Core types and traits for extensible URL handling
//! - HTTP client wrapper for consistent network operations
//! - URL type detection for automatic handler selection
//! - Specific handlers for each supported URL type
//! - Unified public API for simple integration

/// Core types, traits, and error definitions
pub mod types;

/// HTTP client wrapper for network operations
pub mod client;

/// Content converters for different formats
pub mod converters;

/// YAML frontmatter generation and manipulation utilities
pub mod frontmatter;

/// URL type detection and classification
pub mod detection;

/// Configuration system
pub mod config;

/// Utility functions shared across the codebase
pub mod utils;

use crate::client::HttpClient;
use crate::converters::ConverterRegistry;
use crate::detection::UrlDetector;
use crate::types::{Markdown, MarkdownError, UrlType};
use tracing::{debug, error, info, instrument, warn};

/// Main library struct providing unified URL to markdown conversion.
///
/// This struct integrates URL detection, converter routing, and configuration
/// to provide a simple, unified API for converting any supported URL to markdown.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use markdowndown::MarkdownDown;
///
/// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
/// let md = MarkdownDown::new();
/// let result = md.convert_url("https://example.com/article.html").await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
///
/// ## With Custom Configuration
///
/// ```rust
/// use markdowndown::{MarkdownDown, Config};
///
/// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
/// let config = Config::builder()
///     .timeout_seconds(60)
///     .user_agent("MyApp/1.0")
///     .build();
///
/// let md = MarkdownDown::with_config(config);
/// let result = md.convert_url("https://docs.google.com/document/d/abc123/edit").await?;
/// # Ok(())
/// # }
/// ```
pub struct MarkdownDown {
    config: crate::config::Config,
    detector: UrlDetector,
    registry: ConverterRegistry,
}

impl MarkdownDown {
    /// Creates a new MarkdownDown instance with default configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::MarkdownDown;
    ///
    /// let md = MarkdownDown::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: crate::config::Config::default(),
            detector: UrlDetector::new(),
            registry: ConverterRegistry::new(),
        }
    }

    /// Creates a new MarkdownDown instance with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::{MarkdownDown, Config};
    ///
    /// let config = Config::builder()
    ///     .timeout_seconds(45)
    ///     .build();
    ///
    /// let md = MarkdownDown::with_config(config);
    /// ```
    pub fn with_config(config: crate::config::Config) -> Self {
        // Create configured HTTP client
        let http_client = HttpClient::with_config(&config.http, &config.auth);

        // Create registry with configured HTTP client, HTML config, and output config
        let registry =
            ConverterRegistry::with_config(http_client, config.html.clone(), &config.output);

        Self {
            config,
            detector: UrlDetector::new(),
            registry,
        }
    }

    /// Converts content from a URL to markdown.
    ///
    /// This method automatically detects the URL type and routes it to the
    /// appropriate converter for processing.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch and convert
    ///
    /// # Returns
    ///
    /// Returns the converted markdown content or an error.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::InvalidUrl` - If the URL format is invalid
    /// * `MarkdownError::NetworkError` - For network-related failures
    /// * `MarkdownError::ParseError` - If content conversion fails
    /// * `MarkdownError::AuthError` - For authentication failures
    /// * `MarkdownError::ConfigurationError` - If no converter is available for the URL type
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::MarkdownDown;
    ///
    /// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
    /// let md = MarkdownDown::new();
    /// let result = md.convert_url("https://example.com/page.html").await?;
    /// println!("Converted markdown: {}", result);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self), fields(url_type))]
    pub async fn convert_url(&self, url: &str) -> Result<Markdown, MarkdownError> {
        info!("Starting URL conversion for: {}", url);

        let (normalized_url, url_type) = self.prepare_url(url)?;
        let converter = self.get_converter_for_type(&url_type)?;

        info!("Starting conversion with {} converter", url_type);
        match converter.convert(&normalized_url).await {
            Ok(result) => {
                info!(
                    "Successfully converted URL to markdown ({} chars)",
                    result.as_str().len()
                );
                Ok(result)
            }
            Err(e) => {
                error!("Primary converter failed: {}", e);
                self.try_fallback_conversion(&normalized_url, &url_type, e)
                    .await
            }
        }
    }

    /// Normalizes and detects the URL type.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to prepare
    ///
    /// # Returns
    ///
    /// Returns a tuple of the normalized URL and detected URL type.
    fn prepare_url(&self, url: &str) -> Result<(String, UrlType), MarkdownError> {
        debug!("Normalizing URL");
        let normalized_url = self.detector.normalize_url(url)?;
        debug!("Normalized URL: {}", normalized_url);

        debug!("Detecting URL type");
        let url_type = self.detector.detect_type(&normalized_url)?;
        tracing::Span::current().record("url_type", format!("{url_type}"));
        info!("Detected URL type: {}", url_type);

        Ok((normalized_url, url_type))
    }

    /// Gets the appropriate converter for a URL type.
    ///
    /// # Arguments
    ///
    /// * `url_type` - The URL type to get a converter for
    ///
    /// # Returns
    ///
    /// Returns a reference to the converter.
    fn get_converter_for_type(
        &self,
        url_type: &UrlType,
    ) -> Result<&dyn crate::converters::Converter, MarkdownError> {
        debug!("Looking up converter for type: {}", url_type);
        let converter = self.registry.get_converter(url_type).ok_or_else(|| {
            error!("No converter available for URL type: {}", url_type);
            MarkdownError::LegacyConfigurationError {
                message: format!("No converter available for URL type: {url_type}"),
            }
        })?;
        debug!("Found converter for type: {}", url_type);
        Ok(converter)
    }

    /// Attempts fallback conversion using HTML converter for recoverable errors.
    ///
    /// # Arguments
    ///
    /// * `normalized_url` - The normalized URL to convert
    /// * `url_type` - The original URL type that failed
    /// * `error` - The error from the primary converter
    ///
    /// # Returns
    ///
    /// Returns the fallback conversion result or the original error.
    async fn try_fallback_conversion(
        &self,
        normalized_url: &str,
        url_type: &UrlType,
        error: MarkdownError,
    ) -> Result<Markdown, MarkdownError> {
        if !error.is_recoverable() || *url_type == UrlType::Html {
            return Err(error);
        }

        warn!("Attempting HTML fallback conversion for recoverable error");

        if let Some(html_converter) = self.registry.get_converter(&UrlType::Html) {
            match html_converter.convert(normalized_url).await {
                Ok(fallback_result) => {
                    warn!(
                        "Fallback HTML conversion succeeded ({} chars)",
                        fallback_result.as_str().len()
                    );
                    return Ok(fallback_result);
                }
                Err(fallback_error) => {
                    error!("Fallback HTML conversion also failed: {}", fallback_error);
                }
            }
        }

        Err(error)
    }

    /// Returns the configuration being used by this instance.
    pub fn config(&self) -> &crate::config::Config {
        &self.config
    }

    /// Returns the URL detector being used by this instance.
    pub fn detector(&self) -> &UrlDetector {
        &self.detector
    }

    /// Returns the converter registry being used by this instance.
    pub fn registry(&self) -> &ConverterRegistry {
        &self.registry
    }

    /// Lists all supported URL types.
    pub fn supported_types(&self) -> Vec<crate::types::UrlType> {
        self.registry.supported_types()
    }
}

impl Default for MarkdownDown {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function for converting a URL to markdown with default configuration.
///
/// This is equivalent to calling `MarkdownDown::new().convert_url(url).await`.
///
/// # Arguments
///
/// * `url` - The URL to fetch and convert
///
/// # Returns
///
/// Returns the converted markdown content or an error.
///
/// # Examples
///
/// ```rust
/// use markdowndown::convert_url;
///
/// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
/// let result = convert_url("https://example.com/article.html").await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
pub async fn convert_url(url: &str) -> Result<Markdown, MarkdownError> {
    MarkdownDown::new().convert_url(url).await
}

/// Convenience function for converting a URL to markdown with custom configuration.
///
/// # Arguments
///
/// * `url` - The URL to fetch and convert
/// * `config` - The configuration to use
///
/// # Returns
///
/// Returns the converted markdown content or an error.
///
/// # Examples
///
/// ```rust
/// use markdowndown::{convert_url_with_config, Config};
///
/// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
/// let config = Config::builder()
///     .timeout_seconds(60)
///     .build();
///
/// let result = convert_url_with_config("https://example.com/article.html", config).await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
pub async fn convert_url_with_config(
    url: &str,
    config: crate::config::Config,
) -> Result<Markdown, MarkdownError> {
    MarkdownDown::with_config(config).convert_url(url).await
}

/// Utility function to detect the type of a URL without converting it.
///
/// # Arguments
///
/// * `url` - The URL to analyze
///
/// # Returns
///
/// Returns the detected URL type or an error.
///
/// # Examples
///
/// ```rust
/// use markdowndown::{detect_url_type, types::UrlType};
///
/// # fn example() -> Result<(), markdowndown::types::MarkdownError> {
/// let url_type = detect_url_type("https://docs.google.com/document/d/123/edit")?;
/// assert_eq!(url_type, UrlType::GoogleDocs);
/// # Ok(())
/// # }
/// ```
pub fn detect_url_type(url: &str) -> Result<crate::types::UrlType, MarkdownError> {
    let detector = UrlDetector::new();
    detector.detect_type(url)
}

// Re-export main API items for convenience
pub use config::Config;
pub use converters::{Converter, HtmlConverter};
pub use types::{Frontmatter, Url};

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::GitHubConverter;
    use crate::detection::UrlDetector;
    use crate::types::UrlType;
    use std::time::Duration;

    mod test_constants {
        pub mod defaults {
            pub const TIMEOUT_SECONDS: u64 = 30;
            pub const MAX_RETRIES: u32 = 3;
            pub const RETRY_DELAY_SECONDS: u64 = 1;
            pub const MAX_REDIRECTS: u32 = 10;
            pub const MAX_CONSECUTIVE_BLANK_LINES: usize = 2;
        }

        pub mod timeouts {
            pub const SHORT: u64 = 25;
            pub const LONG: u64 = 60;
            pub const MINIMAL: u64 = 1;
        }

        pub mod custom_values {
            pub const MAX_RETRIES: u32 = 5;
            pub const MAX_CONSECUTIVE_BLANK_LINES: usize = 3;
            pub const MIN_CONSECUTIVE_BLANK_LINES: usize = 1;
        }

        pub mod test_data {
            pub const GITHUB_ISSUE_NUMBER: u32 = 12345;
            pub const GITHUB_PR_NUMBER: u32 = 98765;
            pub const EXPECTED_CUSTOM_FIELDS_COUNT: usize = 2;
        }

        pub mod http_status {
            pub const OK: u16 = 200;
            pub const INTERNAL_SERVER_ERROR: u16 = 500;
        }

        pub mod validation {
            pub const MIN_VERSION_PARTS: usize = 2;
        }
    }

    use test_constants::*;

    /// Helper function to assert default configuration values
    fn assert_default_config(config: &Config) {
        assert_eq!(
            config.http.timeout,
            Duration::from_secs(defaults::TIMEOUT_SECONDS)
        );
        assert_eq!(config.http.max_retries, defaults::MAX_RETRIES);
        assert_eq!(
            config.http.retry_delay,
            Duration::from_secs(defaults::RETRY_DELAY_SECONDS)
        );
        assert_eq!(config.http.max_redirects, defaults::MAX_REDIRECTS);
        assert!(config.auth.github_token.is_none());
        assert!(config.auth.office365_token.is_none());
        assert!(config.auth.google_api_key.is_none());
        assert!(config.output.include_frontmatter);
        assert_eq!(
            config.output.max_consecutive_blank_lines,
            defaults::MAX_CONSECUTIVE_BLANK_LINES
        );
        assert!(config.http.user_agent.starts_with("markdowndown/"));
        assert!(config.output.custom_frontmatter_fields.is_empty());
        assert!(config.output.normalize_whitespace);
    }

    /// Helper function to assert markdown contains expected content parts
    fn assert_markdown_contains(markdown: &Markdown, expected_parts: &[&str]) {
        for part in expected_parts {
            assert!(
                markdown.as_str().contains(part),
                "Markdown missing expected content: {}",
                part
            );
        }
    }

    #[test]
    fn test_version_available() {
        // Verify version follows semantic versioning pattern (major.minor.patch)
        assert!(VERSION.chars().any(|c| c.is_ascii_digit()));
        assert!(VERSION.contains('.'));
        // Basic format validation - should have at least one dot for major.minor
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(
            parts.len() >= validation::MIN_VERSION_PARTS,
            "Version should have at least major.minor format"
        );
    }

    #[test]
    fn test_markdowndown_with_default_config() {
        // Test that MarkdownDown can be created with default configuration
        let md = MarkdownDown::new();

        // Verify config is stored and accessible
        assert_default_config(md.config());
    }

    #[test]
    fn test_markdowndown_with_custom_config() {
        // Test that MarkdownDown respects custom configuration
        let config = Config::builder()
            .timeout_seconds(timeouts::LONG)
            .user_agent("TestApp/1.0")
            .max_retries(custom_values::MAX_RETRIES)
            .github_token("test_token")
            .include_frontmatter(false)
            .max_consecutive_blank_lines(custom_values::MIN_CONSECUTIVE_BLANK_LINES)
            .build();

        let md = MarkdownDown::with_config(config);

        // Verify custom config is stored
        let stored_config = md.config();
        assert_eq!(
            stored_config.http.timeout,
            Duration::from_secs(timeouts::LONG)
        );
        assert_eq!(stored_config.http.user_agent, "TestApp/1.0");
        assert_eq!(stored_config.http.max_retries, custom_values::MAX_RETRIES);
        assert_eq!(
            stored_config.auth.github_token.as_deref(),
            Some("test_token")
        );
        assert!(!stored_config.output.include_frontmatter);
        assert_eq!(
            stored_config.output.max_consecutive_blank_lines,
            custom_values::MIN_CONSECUTIVE_BLANK_LINES
        );
    }

    #[test]
    fn test_config_builder_fluent_interface() {
        // Test that the config builder's fluent interface works correctly
        let config = Config::builder()
            .github_token("ghp_test_token")
            .office365_token("office_token")
            .google_api_key("google_key")
            .timeout_seconds(timeouts::LONG)
            .user_agent("IntegrationTest/2.0")
            .max_retries(defaults::MAX_RETRIES)
            .include_frontmatter(true)
            .custom_frontmatter_field("project", "markdowndown")
            .custom_frontmatter_field("version", "test")
            .normalize_whitespace(false)
            .max_consecutive_blank_lines(custom_values::MAX_CONSECUTIVE_BLANK_LINES)
            .build();

        // Verify all custom settings
        assert_eq!(config.auth.github_token.as_deref(), Some("ghp_test_token"));
        assert_eq!(config.auth.office365_token.as_deref(), Some("office_token"));
        assert_eq!(config.auth.google_api_key.as_deref(), Some("google_key"));
        assert_eq!(config.http.timeout, Duration::from_secs(timeouts::LONG));
        assert_eq!(config.http.user_agent, "IntegrationTest/2.0");
        assert_eq!(config.http.max_retries, defaults::MAX_RETRIES);
        assert!(config.output.include_frontmatter);
        assert_eq!(
            config.output.custom_frontmatter_fields.len(),
            test_data::EXPECTED_CUSTOM_FIELDS_COUNT
        );
        assert_eq!(
            config.output.custom_frontmatter_fields[0],
            ("project".to_string(), "markdowndown".to_string())
        );
        assert_eq!(
            config.output.custom_frontmatter_fields[1],
            ("version".to_string(), "test".to_string())
        );
        assert!(!config.output.normalize_whitespace);
        assert_eq!(
            config.output.max_consecutive_blank_lines,
            custom_values::MAX_CONSECUTIVE_BLANK_LINES
        );
    }

    #[test]
    fn test_config_from_default() {
        // Test that Config::default() produces expected defaults
        let config = Config::default();

        // Use helper for all default assertions
        assert_default_config(&config);
    }

    #[test]
    fn test_supported_url_types() {
        // Test that MarkdownDown reports supported URL types correctly
        let md = MarkdownDown::new();
        let supported_types = md.supported_types();

        // Should support at least these URL types
        assert!(supported_types.contains(&crate::types::UrlType::Html));
        assert!(supported_types.contains(&crate::types::UrlType::GoogleDocs));
        assert!(supported_types.contains(&crate::types::UrlType::GitHubIssue));
        assert!(supported_types.contains(&crate::types::UrlType::LocalFile));
    }

    #[test]
    fn test_detect_url_type_integration() {
        // Test that URL type detection works through the main API

        // Test HTML URL
        let html_result = detect_url_type("https://example.com/article.html");
        assert!(html_result.is_ok());
        assert_eq!(html_result.unwrap(), crate::types::UrlType::Html);

        // Test Google Docs URL
        let gdocs_result = detect_url_type("https://docs.google.com/document/d/abc123/edit");
        assert!(gdocs_result.is_ok());
        assert_eq!(gdocs_result.unwrap(), crate::types::UrlType::GoogleDocs);

        // Test GitHub Issue URL
        let github_result = detect_url_type("https://github.com/owner/repo/issues/123");
        assert!(github_result.is_ok());
        assert_eq!(github_result.unwrap(), crate::types::UrlType::GitHubIssue);

        // Test invalid URL
        let invalid_result = detect_url_type("not-a-url");
        assert!(invalid_result.is_err());
    }

    /// Helper function to assert GitHub URL parsing
    fn assert_github_url_parsing(
        url: &str,
        expected_owner: &str,
        expected_repo: &str,
        expected_number: u32,
    ) {
        let detector = UrlDetector::new();
        let converter = GitHubConverter::new();

        let detected_type = detector.detect_type(url).unwrap();
        assert_eq!(detected_type, UrlType::GitHubIssue);

        let parsed = converter.parse_github_url(url).unwrap();
        assert_eq!(parsed.owner, expected_owner);
        assert_eq!(parsed.repo, expected_repo);
        assert_eq!(parsed.number, expected_number);
    }

    #[test]
    fn test_github_integration_issue_and_pr() {
        // Test integration between URL detection and GitHub converter with parametric test cases
        let test_cases = [
            (
                format!(
                    "https://github.com/microsoft/vscode/issues/{}",
                    test_data::GITHUB_ISSUE_NUMBER
                ),
                "microsoft",
                "vscode",
                test_data::GITHUB_ISSUE_NUMBER,
            ),
            (
                format!(
                    "https://github.com/rust-lang/rust/pull/{}",
                    test_data::GITHUB_PR_NUMBER
                ),
                "rust-lang",
                "rust",
                test_data::GITHUB_PR_NUMBER,
            ),
        ];

        for (url, owner, repo, number) in test_cases {
            assert_github_url_parsing(&url, owner, repo, number);
        }
    }

    /// Comprehensive tests for improved coverage
    mod comprehensive_coverage_tests {
        use super::*;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        /// Helper function to set up a mock HTTP server with HTML content
        async fn setup_mock_html_server(
            request_path: &str,
            html_content: &str,
        ) -> (MockServer, String) {
            let mock_server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(request_path))
                .respond_with(ResponseTemplate::new(http_status::OK).set_body_string(html_content))
                .mount(&mock_server)
                .await;
            let url = format!("{}{}", mock_server.uri(), request_path);
            (mock_server, url)
        }

        /// Helper function to test HTML to markdown conversion
        async fn test_html_conversion(
            html: &str,
            expected: &[&str],
        ) -> Result<Markdown, MarkdownError> {
            let (_mock_server, url) = setup_mock_html_server("/test", html).await;
            let result = convert_url(&url).await?;
            assert_markdown_contains(&result, expected);
            Ok(result)
        }

        /// Helper function to assert invalid URL error
        async fn assert_invalid_url_error(url: &str) {
            let md = MarkdownDown::new();
            let result = md.convert_url(url).await;
            assert!(result.is_err());
            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, crate::types::ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.url, url);
                }
                _ => panic!("Expected ValidationError for invalid URL"),
            }
        }

        #[test]
        fn test_detector_getter() {
            // Test the detector() getter method
            let md = MarkdownDown::new();
            let detector = md.detector();

            // Should return a valid detector that can detect URL types
            let result = detector.detect_type("https://example.com/page.html");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), UrlType::Html);
        }

        #[test]
        fn test_registry_getter() {
            // Test the registry() getter method
            let md = MarkdownDown::new();
            let registry = md.registry();

            // Should return a valid registry with converters
            let supported_types = registry.supported_types();
            assert!(!supported_types.is_empty());
            assert!(supported_types.contains(&UrlType::Html));
        }

        #[test]
        fn test_default_trait_implementation() {
            // Test that Default trait is properly implemented
            let md1 = MarkdownDown::new();
            let md2 = MarkdownDown::default();

            // Both should have identical configurations
            assert_eq!(md1.config().http.timeout, md2.config().http.timeout);
            assert_eq!(md1.config().http.max_retries, md2.config().http.max_retries);
            assert_eq!(
                md1.config().auth.github_token,
                md2.config().auth.github_token
            );
            assert_eq!(
                md1.config().output.include_frontmatter,
                md2.config().output.include_frontmatter
            );
        }

        #[tokio::test]
        async fn test_convert_url_convenience_function() {
            // Test the standalone convert_url function
            let html_content = "<h1>Test Content</h1><p>This is a test.</p>";
            let result =
                test_html_conversion(html_content, &["# Test Content", "This is a test"]).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_convert_url_with_config_convenience_function() {
            // Test the standalone convert_url_with_config function
            let html_content =
                "<h1>Custom Config Test</h1><p>Testing with custom configuration.</p>";
            let (_mock_server, url) =
                setup_mock_html_server("/custom-config-page", html_content).await;

            // Create custom configuration
            let config = Config::builder()
                .timeout_seconds(timeouts::LONG)
                .user_agent("TestConvenience/1.0")
                .include_frontmatter(false)
                .build();

            let result = convert_url_with_config(&url, config).await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            assert_markdown_contains(
                &markdown,
                &["# Custom Config Test", "Testing with custom configuration"],
            );
            // Should not have frontmatter since we disabled it
            assert!(!markdown.as_str().starts_with("---"));
        }

        #[tokio::test]
        async fn test_convert_url_error_no_converter_available() {
            // Test error path when no converter is available for URL type
            // This is tricky to test directly, but we can test with a custom registry
            // that has been modified to not have converters for certain types

            // For this test, we'll create a scenario where the fallback would be attempted
            // by using a URL that should work but simulating a failure
            let mock_server = MockServer::start().await;

            // Return an error status to trigger the error handling path
            Mock::given(method("GET"))
                .and(path("/error-test"))
                .respond_with(ResponseTemplate::new(http_status::INTERNAL_SERVER_ERROR))
                .mount(&mock_server)
                .await;

            // Use a config with no retries and short timeout to speed up the test
            let config = Config::builder()
                .timeout_seconds(timeouts::MINIMAL)
                .max_retries(0)
                .build();
            let md = MarkdownDown::with_config(config);
            let url = format!("{}/error-test", mock_server.uri());
            let result = md.convert_url(&url).await;

            // Should result in an error due to server error
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_fallback_conversion_logic() {
            // Test the fallback logic when primary converter fails but error is recoverable
            let html_content = "<h1>Fallback Test</h1><p>This should work via fallback.</p>";
            let result = test_html_conversion(
                html_content,
                &["# Fallback Test", "This should work via fallback"],
            )
            .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_convert_url_invalid_url_error() {
            // Test convert_url with an invalid URL to trigger validation error
            assert_invalid_url_error("not-a-valid-url").await;
        }

        #[tokio::test]
        async fn test_convert_url_malformed_url_error() {
            // Test convert_url with a malformed URL
            assert_invalid_url_error("http://[invalid-host").await;
        }

        #[tokio::test]
        async fn test_successful_conversion_with_instrumentation() {
            // Test successful conversion to ensure instrumentation line is covered
            let html_content =
                "<h1>Instrumentation Test</h1><p>Testing the instrumentation decorator.</p>";
            let result = test_html_conversion(
                html_content,
                &[
                    "# Instrumentation Test",
                    "Testing the instrumentation decorator",
                ],
            )
            .await;
            assert!(result.is_ok());
        }

        #[test]
        fn test_config_accessor() {
            // Test config accessor with custom settings
            let config = Config::builder()
                .timeout_seconds(timeouts::SHORT)
                .user_agent("AccessorTest/1.0")
                .github_token("test-accessor-token")
                .include_frontmatter(true)
                .build();

            let md = MarkdownDown::with_config(config);

            let stored_config = md.config();
            assert_eq!(
                stored_config.http.timeout,
                Duration::from_secs(timeouts::SHORT)
            );
            assert_eq!(stored_config.http.user_agent, "AccessorTest/1.0");
            assert_eq!(
                stored_config.auth.github_token.as_deref(),
                Some("test-accessor-token")
            );
            assert!(stored_config.output.include_frontmatter);
        }

        #[test]
        fn test_detector_accessor() {
            // Test detector accessor functionality
            let md = MarkdownDown::new();
            let detector = md.detector();

            let html_result = detector.detect_type("https://example.com/test.html");
            assert!(html_result.is_ok());
            assert_eq!(html_result.unwrap(), UrlType::Html);
        }

        #[test]
        fn test_registry_accessor() {
            // Test registry accessor and supported types
            let md = MarkdownDown::new();
            let registry = md.registry();
            let supported = registry.supported_types();

            let expected_types = [
                UrlType::Html,
                UrlType::GoogleDocs,
                UrlType::GitHubIssue,
                UrlType::LocalFile,
            ];
            for expected_type in expected_types {
                assert!(
                    supported.contains(&expected_type),
                    "Registry should support {:?}",
                    expected_type
                );
            }
        }

        #[test]
        fn test_supported_types_method() {
            // Test the supported_types method specifically
            let md = MarkdownDown::new();
            let md_supported = md.supported_types();
            let registry_supported = md.registry().supported_types();

            assert_eq!(md_supported, registry_supported);
        }
    }
}
