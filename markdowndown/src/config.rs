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
            max_consecutive_blank_lines: 2,
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
                timeout: Duration::from_secs(30),
                user_agent: format!("markdowndown/{}", env!("CARGO_PKG_VERSION")),
                max_retries: 3,
                retry_delay: Duration::from_secs(1),
                max_redirects: 10,
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
                max_consecutive_blank_lines: 2,
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
        assert_eq!(builder.http.timeout, Duration::from_secs(30));
        assert_eq!(builder.http.max_retries, 3);
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
        assert_eq!(config.http.timeout, Duration::from_secs(30));
        assert_eq!(config.http.max_retries, 3);
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
        let config = Config::from_env();

        // Should have default values
        assert_eq!(config.http.timeout, Duration::from_secs(30));
        assert_eq!(config.http.max_retries, 3);
        assert!(config.auth.github_token.is_none());
    }

    // Note: Testing actual environment variables would require setting them,
    // which could interfere with other tests. In practice, these would be
    // integration tests or tested with environment variable mocking.
}
