//! Configuration system for the markdowndown library.
//!
//! This module provides comprehensive configuration options for all converters,
//! HTTP client settings, authentication, and output formatting. It uses the
//! builder pattern for easy and flexible configuration setup.
//!
//! # Usage Examples
//!
//! ## Default Configuration
//!
//! ```rust
//! use markdowndown::Config;
//!
//! let config = Config::default();
//! ```
//!
//! ## Custom Configuration
//!
//! ```rust
//! use markdowndown::Config;
//!
//! let config = Config::builder()
//!     .github_token("ghp_xxxxxxxxxxxxxxxxxxxx")
//!     .timeout_seconds(60)
//!     .user_agent("MyApp/1.0")
//!     .max_retries(5)
//!     .build();
//! ```
//!
//! ## Environment-based Configuration
//!
//! ```rust
//! use markdowndown::Config;
//!
//! let config = Config::from_env();
//! ```

use crate::converters::html::HtmlConverterConfig;
use std::time::Duration;

// Default configuration constants
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_RETRY_DELAY_SECONDS: u64 = 1;
const DEFAULT_MAX_REDIRECTS: u32 = 10;
const DEFAULT_MAX_CONSECUTIVE_BLANK_LINES: usize = 2;

/// Main configuration struct for the markdowndown library.
///
/// This struct contains all configuration options for HTTP client settings,
/// authentication tokens, converter-specific options, and output formatting.
#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP client configuration
    pub http: HttpConfig,
    /// Authentication tokens for various services
    pub auth: AuthConfig,
    /// HTML converter specific settings
    pub html: HtmlConverterConfig,
    /// Output formatting options
    pub output: OutputConfig,
}

/// HTTP client configuration options.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Request timeout duration
    pub timeout: Duration,
    /// User agent string for HTTP requests
    pub user_agent: String,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries
    pub retry_delay: Duration,
    /// Maximum number of redirects to follow
    pub max_redirects: u32,
}

/// Authentication configuration for various services.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// GitHub personal access token
    pub github_token: Option<String>,
    /// Office 365 authentication token (placeholder for future use)
    pub office365_token: Option<String>,
    /// Google API key (placeholder for future use)
    pub google_api_key: Option<String>,
}

/// Output formatting configuration.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Whether to include YAML frontmatter in output
    pub include_frontmatter: bool,
    /// Custom frontmatter fields to include
    pub custom_frontmatter_fields: Vec<(String, String)>,
    /// Whether to normalize whitespace in output
    pub normalize_whitespace: bool,
    /// Maximum blank lines to allow consecutively
    pub max_consecutive_blank_lines: usize,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            include_frontmatter: true,
            custom_frontmatter_fields: Vec::new(),
            normalize_whitespace: true,
            max_consecutive_blank_lines: DEFAULT_MAX_CONSECUTIVE_BLANK_LINES,
        }
    }
}

/// Builder for creating Config instances with a fluent interface.
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    http: HttpConfig,
    auth: AuthConfig,
    html: HtmlConverterConfig,
    output: OutputConfig,
}

impl Config {
    /// Creates a new configuration builder.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .github_token("token")
    ///     .timeout_seconds(30)
    ///     .build();
    /// ```
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Creates configuration from environment variables.
    ///
    /// This method looks for the following environment variables:
    /// - `GITHUB_TOKEN` - GitHub personal access token
    /// - `MARKDOWNDOWN_TIMEOUT` - HTTP timeout in seconds
    /// - `MARKDOWNDOWN_USER_AGENT` - Custom user agent string
    /// - `MARKDOWNDOWN_MAX_RETRIES` - Maximum retry attempts
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// // Set environment variables first:
    /// // export GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx
    /// // export MARKDOWNDOWN_TIMEOUT=60
    ///
    /// let config = Config::from_env();
    /// ```
    pub fn from_env() -> Self {
        let mut builder = ConfigBuilder::new();

        // Load GitHub token from environment
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            if !token.trim().is_empty() {
                builder = builder.github_token(token);
            }
        }

        // Load timeout from environment
        if let Ok(timeout_str) = std::env::var("MARKDOWNDOWN_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                builder = builder.timeout_seconds(timeout_secs);
            }
        }

        // Load user agent from environment
        if let Ok(user_agent) = std::env::var("MARKDOWNDOWN_USER_AGENT") {
            if !user_agent.trim().is_empty() {
                builder = builder.user_agent(user_agent);
            }
        }

        // Load max retries from environment
        if let Ok(retries_str) = std::env::var("MARKDOWNDOWN_MAX_RETRIES") {
            if let Ok(retries) = retries_str.parse::<u32>() {
                builder = builder.max_retries(retries);
            }
        }

        builder.build()
    }
}

impl Default for Config {
    fn default() -> Self {
        ConfigBuilder::new().build()
    }
}

impl ConfigBuilder {
    /// Creates a new configuration builder with default values.
    pub fn new() -> Self {
        Self {
            http: HttpConfig {
                timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
                user_agent: format!("markdowndown/{}", env!("CARGO_PKG_VERSION")),
                max_retries: DEFAULT_MAX_RETRIES,
                retry_delay: Duration::from_secs(DEFAULT_RETRY_DELAY_SECONDS),
                max_redirects: DEFAULT_MAX_REDIRECTS,
            },
            auth: AuthConfig {
                github_token: None,
                office365_token: None,
                google_api_key: None,
            },
            html: HtmlConverterConfig::default(),
            output: OutputConfig {
                include_frontmatter: true,
                custom_frontmatter_fields: Vec::new(),
                normalize_whitespace: true,
                max_consecutive_blank_lines: DEFAULT_MAX_CONSECUTIVE_BLANK_LINES,
            },
        }
    }

    /// Sets the GitHub personal access token.
    ///
    /// # Arguments
    ///
    /// * `token` - GitHub personal access token (starts with ghp_ or github_pat_)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .github_token("ghp_xxxxxxxxxxxxxxxxxxxx")
    ///     .build();
    /// ```
    pub fn github_token<T: Into<String>>(mut self, token: T) -> Self {
        self.auth.github_token = Some(token.into());
        self
    }

    /// Sets the Office 365 authentication token (placeholder for future use).
    ///
    /// # Arguments
    ///
    /// * `token` - Office 365 authentication token
    pub fn office365_token<T: Into<String>>(mut self, token: T) -> Self {
        self.auth.office365_token = Some(token.into());
        self
    }

    /// Sets the Google API key (placeholder for future use).
    ///
    /// # Arguments
    ///
    /// * `key` - Google API key
    pub fn google_api_key<T: Into<String>>(mut self, key: T) -> Self {
        self.auth.google_api_key = Some(key.into());
        self
    }

    /// Sets the HTTP request timeout in seconds.
    ///
    /// # Arguments
    ///
    /// * `seconds` - Timeout in seconds (converted to Duration)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .timeout_seconds(60)
    ///     .build();
    /// ```
    pub fn timeout_seconds(mut self, seconds: u64) -> Self {
        self.http.timeout = Duration::from_secs(seconds);
        self
    }

    /// Sets the HTTP request timeout as a Duration.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Timeout duration
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.http.timeout = timeout;
        self
    }

    /// Sets the User-Agent header for HTTP requests.
    ///
    /// # Arguments
    ///
    /// * `user_agent` - User agent string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .user_agent("MyApp/1.0")
    ///     .build();
    /// ```
    pub fn user_agent<T: Into<String>>(mut self, user_agent: T) -> Self {
        self.http.user_agent = user_agent.into();
        self
    }

    /// Sets the maximum number of retry attempts for failed requests.
    ///
    /// # Arguments
    ///
    /// * `retries` - Maximum number of retries (0 disables retries)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .max_retries(5)
    ///     .build();
    /// ```
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.http.max_retries = retries;
        self
    }

    /// Sets the base delay between retry attempts.
    ///
    /// # Arguments
    ///
    /// * `delay` - Base delay duration (actual delay uses exponential backoff)
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.http.retry_delay = delay;
        self
    }

    /// Sets the maximum number of HTTP redirects to follow.
    ///
    /// # Arguments
    ///
    /// * `redirects` - Maximum redirects (0 disables redirect following)
    pub fn max_redirects(mut self, redirects: u32) -> Self {
        self.http.max_redirects = redirects;
        self
    }

    /// Sets HTML converter configuration.
    ///
    /// # Arguments
    ///
    /// * `html_config` - HTML converter configuration
    pub fn html_config(mut self, html_config: HtmlConverterConfig) -> Self {
        self.html = html_config;
        self
    }

    /// Sets whether to include YAML frontmatter in output.
    ///
    /// # Arguments
    ///
    /// * `include` - Whether to include frontmatter
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .include_frontmatter(false)
    ///     .build();
    /// ```
    pub fn include_frontmatter(mut self, include: bool) -> Self {
        self.output.include_frontmatter = include;
        self
    }

    /// Adds a custom frontmatter field.
    ///
    /// # Arguments
    ///
    /// * `key` - Frontmatter field name
    /// * `value` - Frontmatter field value
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .custom_frontmatter_field("project", "my-project")
    ///     .custom_frontmatter_field("version", "1.0")
    ///     .build();
    /// ```
    pub fn custom_frontmatter_field<K: Into<String>, V: Into<String>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.output
            .custom_frontmatter_fields
            .push((key.into(), value.into()));
        self
    }

    /// Sets whether to normalize whitespace in output.
    ///
    /// # Arguments
    ///
    /// * `normalize` - Whether to normalize whitespace
    pub fn normalize_whitespace(mut self, normalize: bool) -> Self {
        self.output.normalize_whitespace = normalize;
        self
    }

    /// Sets the maximum number of consecutive blank lines allowed.
    ///
    /// # Arguments
    ///
    /// * `lines` - Maximum consecutive blank lines
    pub fn max_consecutive_blank_lines(mut self, lines: usize) -> Self {
        self.output.max_consecutive_blank_lines = lines;
        self
    }

    /// Builds the final configuration.
    ///
    /// # Returns
    ///
    /// A fully configured `Config` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::Config;
    ///
    /// let config = Config::builder()
    ///     .github_token("token")
    ///     .timeout_seconds(30)
    ///     .build();
    /// ```
    pub fn build(self) -> Config {
        Config {
            http: self.http,
            auth: self.auth,
            html: self.html,
            output: self.output,
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_new() {
        let builder = ConfigBuilder::new();
        assert_eq!(
            builder.http.timeout,
            Duration::from_secs(DEFAULT_TIMEOUT_SECONDS)
        );
        assert_eq!(builder.http.max_retries, DEFAULT_MAX_RETRIES);
        assert!(builder.auth.github_token.is_none());
        assert!(builder.output.include_frontmatter);
    }

    #[test]
    fn test_config_builder_github_token() {
        let config = ConfigBuilder::new().github_token("ghp_test_token").build();

        assert_eq!(config.auth.github_token, Some("ghp_test_token".to_string()));
    }

    #[test]
    fn test_config_builder_timeout() {
        let config = ConfigBuilder::new().timeout_seconds(60).build();

        assert_eq!(config.http.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_builder_user_agent() {
        let config = ConfigBuilder::new().user_agent("TestApp/1.0").build();

        assert_eq!(config.http.user_agent, "TestApp/1.0");
    }

    #[test]
    fn test_config_builder_retries() {
        let config = ConfigBuilder::new().max_retries(5).build();

        assert_eq!(config.http.max_retries, 5);
    }

    #[test]
    fn test_config_builder_frontmatter() {
        let config = ConfigBuilder::new().include_frontmatter(false).build();

        assert!(!config.output.include_frontmatter);
    }

    #[test]
    fn test_config_builder_custom_frontmatter_fields() {
        let config = ConfigBuilder::new()
            .custom_frontmatter_field("project", "test")
            .custom_frontmatter_field("version", "1.0")
            .build();

        assert_eq!(config.output.custom_frontmatter_fields.len(), 2);
        assert_eq!(
            config.output.custom_frontmatter_fields[0],
            ("project".to_string(), "test".to_string())
        );
        assert_eq!(
            config.output.custom_frontmatter_fields[1],
            ("version".to_string(), "1.0".to_string())
        );
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(
            config.http.timeout,
            Duration::from_secs(DEFAULT_TIMEOUT_SECONDS)
        );
        assert_eq!(config.http.max_retries, DEFAULT_MAX_RETRIES);
        assert!(config.auth.github_token.is_none());
        assert!(config.output.include_frontmatter);
    }

    #[test]
    fn test_config_builder_fluent_interface() {
        let config = Config::builder()
            .github_token("token")
            .timeout_seconds(45)
            .user_agent("FluentTest/1.0")
            .max_retries(2)
            .include_frontmatter(false)
            .custom_frontmatter_field("app", "test")
            .build();

        assert_eq!(config.auth.github_token, Some("token".to_string()));
        assert_eq!(config.http.timeout, Duration::from_secs(45));
        assert_eq!(config.http.user_agent, "FluentTest/1.0");
        assert_eq!(config.http.max_retries, 2);
        assert!(!config.output.include_frontmatter);
        assert_eq!(config.output.custom_frontmatter_fields.len(), 1);
    }

    #[test]
    fn test_config_from_env_no_vars() {
        // Test with no environment variables set
        // Clear any variables that might be set
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("MARKDOWNDOWN_TIMEOUT");
        std::env::remove_var("MARKDOWNDOWN_USER_AGENT");
        std::env::remove_var("MARKDOWNDOWN_MAX_RETRIES");

        let config = Config::from_env();

        // Should have default values
        assert_eq!(
            config.http.timeout,
            Duration::from_secs(DEFAULT_TIMEOUT_SECONDS)
        );
        assert_eq!(config.http.max_retries, DEFAULT_MAX_RETRIES);
        assert!(config.auth.github_token.is_none());
    }

    #[test]
    fn test_config_from_env_github_token() {
        // Set GITHUB_TOKEN environment variable
        std::env::set_var("GITHUB_TOKEN", "ghp_test_token_from_env");

        let config = Config::from_env();

        assert_eq!(
            config.auth.github_token,
            Some("ghp_test_token_from_env".to_string())
        );

        // Clean up
        std::env::remove_var("GITHUB_TOKEN");
    }

    #[test]
    fn test_config_from_env_github_token_empty() {
        // Set GITHUB_TOKEN to empty string - should be ignored
        std::env::set_var("GITHUB_TOKEN", "   ");

        let config = Config::from_env();

        // Empty/whitespace-only tokens should be ignored
        assert!(config.auth.github_token.is_none());

        // Clean up
        std::env::remove_var("GITHUB_TOKEN");
    }

    #[test]
    fn test_config_from_env_timeout() {
        // Set MARKDOWNDOWN_TIMEOUT environment variable
        std::env::set_var("MARKDOWNDOWN_TIMEOUT", "120");

        let config = Config::from_env();

        assert_eq!(config.http.timeout, Duration::from_secs(120));

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_TIMEOUT");
    }

    #[test]
    fn test_config_from_env_timeout_invalid() {
        // Set MARKDOWNDOWN_TIMEOUT to invalid value - should use default
        std::env::set_var("MARKDOWNDOWN_TIMEOUT", "not_a_number");

        let config = Config::from_env();

        // Should fall back to default
        assert_eq!(
            config.http.timeout,
            Duration::from_secs(DEFAULT_TIMEOUT_SECONDS)
        );

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_TIMEOUT");
    }

    #[test]
    fn test_config_from_env_user_agent() {
        // Set MARKDOWNDOWN_USER_AGENT environment variable
        std::env::set_var("MARKDOWNDOWN_USER_AGENT", "CustomAgent/2.0");

        let config = Config::from_env();

        assert_eq!(config.http.user_agent, "CustomAgent/2.0");

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_USER_AGENT");
    }

    #[test]
    fn test_config_from_env_user_agent_empty() {
        // Set MARKDOWNDOWN_USER_AGENT to empty string - should use default
        std::env::set_var("MARKDOWNDOWN_USER_AGENT", "  ");

        let config = Config::from_env();

        // Empty/whitespace-only user agents should be ignored, using default
        assert_eq!(
            config.http.user_agent,
            format!("markdowndown/{}", env!("CARGO_PKG_VERSION"))
        );

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_USER_AGENT");
    }

    #[test]
    fn test_config_from_env_max_retries() {
        // Set MARKDOWNDOWN_MAX_RETRIES environment variable
        std::env::set_var("MARKDOWNDOWN_MAX_RETRIES", "10");

        let config = Config::from_env();

        assert_eq!(config.http.max_retries, 10);

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_MAX_RETRIES");
    }

    #[test]
    fn test_config_from_env_max_retries_invalid() {
        // Set MARKDOWNDOWN_MAX_RETRIES to invalid value - should use default
        std::env::set_var("MARKDOWNDOWN_MAX_RETRIES", "invalid");

        let config = Config::from_env();

        // Should fall back to default
        assert_eq!(config.http.max_retries, DEFAULT_MAX_RETRIES);

        // Clean up
        std::env::remove_var("MARKDOWNDOWN_MAX_RETRIES");
    }

    #[test]
    fn test_config_from_env_all_vars() {
        // Set all environment variables
        std::env::set_var("GITHUB_TOKEN", "ghp_full_test_token");
        std::env::set_var("MARKDOWNDOWN_TIMEOUT", "90");
        std::env::set_var("MARKDOWNDOWN_USER_AGENT", "TestApp/3.0");
        std::env::set_var("MARKDOWNDOWN_MAX_RETRIES", "7");

        let config = Config::from_env();

        // Verify all values were loaded correctly
        assert_eq!(
            config.auth.github_token,
            Some("ghp_full_test_token".to_string())
        );
        assert_eq!(config.http.timeout, Duration::from_secs(90));
        assert_eq!(config.http.user_agent, "TestApp/3.0");
        assert_eq!(config.http.max_retries, 7);

        // Clean up
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("MARKDOWNDOWN_TIMEOUT");
        std::env::remove_var("MARKDOWNDOWN_USER_AGENT");
        std::env::remove_var("MARKDOWNDOWN_MAX_RETRIES");
    }

    #[test]
    fn test_config_builder_office365_token() {
        let config = ConfigBuilder::new()
            .office365_token("o365_test_token")
            .build();

        assert_eq!(
            config.auth.office365_token,
            Some("o365_test_token".to_string())
        );
    }

    #[test]
    fn test_config_builder_google_api_key() {
        let config = ConfigBuilder::new()
            .google_api_key("google_test_key")
            .build();

        assert_eq!(
            config.auth.google_api_key,
            Some("google_test_key".to_string())
        );
    }

    #[test]
    fn test_config_builder_timeout_duration() {
        let config = ConfigBuilder::new()
            .timeout(Duration::from_secs(90))
            .build();

        assert_eq!(config.http.timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_config_builder_retry_delay() {
        let config = ConfigBuilder::new()
            .retry_delay(Duration::from_secs(5))
            .build();

        assert_eq!(config.http.retry_delay, Duration::from_secs(5));
    }

    #[test]
    fn test_config_builder_max_redirects() {
        let config = ConfigBuilder::new().max_redirects(20).build();

        assert_eq!(config.http.max_redirects, 20);
    }

    #[test]
    fn test_config_builder_html_config() {
        let html_config = HtmlConverterConfig::default();
        let config = ConfigBuilder::new()
            .html_config(html_config.clone())
            .build();

        // Verify that html config was set - comparing some fields from HtmlConverterConfig
        assert_eq!(config.html.max_line_width, html_config.max_line_width);
        assert_eq!(
            config.html.remove_scripts_styles,
            html_config.remove_scripts_styles
        );
    }

    #[test]
    fn test_config_builder_normalize_whitespace() {
        let config = ConfigBuilder::new().normalize_whitespace(false).build();

        assert!(!config.output.normalize_whitespace);
    }

    #[test]
    fn test_config_builder_max_consecutive_blank_lines() {
        let config = ConfigBuilder::new().max_consecutive_blank_lines(5).build();

        assert_eq!(config.output.max_consecutive_blank_lines, 5);
    }
}
