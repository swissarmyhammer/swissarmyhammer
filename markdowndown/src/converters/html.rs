//! HTML to markdown conversion with preprocessing and cleanup.
//!
//! This module provides robust HTML to markdown conversion using html2text
//! with intelligent preprocessing to remove unwanted elements and postprocessing
//! to clean up the markdown output.

use crate::client::HttpClient;
use crate::frontmatter::FrontmatterBuilder;
use crate::types::{Markdown, MarkdownError};
use async_trait::async_trait;
use chrono::Utc;
use html2text::from_read;
use std::io::Cursor;

pub use super::config::HtmlConverterConfig;
use super::converter::Converter;
use super::postprocessor::MarkdownPostprocessor;
use super::preprocessor::HtmlPreprocessor;

/// HTML to markdown converter with intelligent preprocessing and cleanup.
#[derive(Debug, Clone)]
pub struct HtmlConverter {
    config: HtmlConverterConfig,
    output_config: crate::config::OutputConfig,
    client: HttpClient,
}

impl HtmlConverter {
    /// Creates a new HTML converter with default configuration.
    ///
    /// # Returns
    ///
    /// A new `HtmlConverter` instance with sensible defaults for most use cases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::HtmlConverter;
    ///
    /// let converter = HtmlConverter::new();
    /// // Use converter.convert(url) to convert HTML from URL to markdown
    /// ```
    pub fn new() -> Self {
        Self {
            config: HtmlConverterConfig::default(),
            output_config: crate::config::OutputConfig::default(),
            client: HttpClient::new(),
        }
    }

    /// Creates a new HTML converter with custom configuration and HTTP client.
    ///
    /// # Arguments
    ///
    /// * `client` - Configured HTTP client to use for requests
    /// * `config` - Custom configuration options for the converter
    /// * `output_config` - Output configuration including custom frontmatter fields
    ///
    /// # Returns
    ///
    /// A new `HtmlConverter` instance with the specified configuration.
    pub fn with_config(
        client: HttpClient,
        config: HtmlConverterConfig,
        output_config: crate::config::OutputConfig,
    ) -> Self {
        Self {
            config,
            output_config,
            client,
        }
    }

    /// Creates a new HTML converter with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom configuration options for the converter
    ///
    /// # Returns
    ///
    /// A new `HtmlConverter` instance with the specified configuration and default HTTP client.
    pub fn with_config_only(config: HtmlConverterConfig) -> Self {
        Self {
            config,
            output_config: crate::config::OutputConfig::default(),
            client: HttpClient::new(),
        }
    }

    /// Converts HTML to clean markdown with preprocessing and postprocessing.
    ///
    /// This method implements a complete pipeline:
    /// 1. Preprocess HTML to remove unwanted elements
    /// 2. Convert HTML to markdown using html2text
    /// 3. Postprocess markdown to clean up formatting
    ///
    /// # Arguments
    ///
    /// * `html` - The HTML content to convert
    ///
    /// # Returns
    ///
    /// Returns clean markdown content on success, or a `MarkdownError` on failure.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::ParseError` - If HTML parsing or conversion fails
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::HtmlConverter;
    ///
    /// let converter = HtmlConverter::new();
    /// let html = "<h1>Hello World</h1><p>This is a test.</p>";
    /// let markdown = converter.convert_html(html)?;
    /// assert!(markdown.contains("# Hello World"));
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn convert_html(&self, html: &str) -> Result<String, MarkdownError> {
        // Validate input
        if html.trim().is_empty() {
            return Err(MarkdownError::ParseError {
                message: format!(
                    "HTML content cannot be empty (received {} characters of whitespace/empty content)",
                    html.len()
                ),
            });
        }

        // Step 1: Preprocess HTML
        let preprocessor = HtmlPreprocessor::new(&self.config);
        let cleaned_html = preprocessor.preprocess(html);

        // Step 2: Convert to markdown
        let markdown = self.html_to_markdown(&cleaned_html).map_err(|e| {
            if let MarkdownError::ParseError { message } = e {
                MarkdownError::ParseError {
                    message: format!(
                        "Failed to convert HTML to markdown (HTML length: {} chars): {}",
                        cleaned_html.len(),
                        message
                    ),
                }
            } else {
                e
            }
        })?;

        // Step 3: Postprocess markdown
        let postprocessor = MarkdownPostprocessor::new(&self.config);
        let cleaned_markdown = postprocessor.postprocess(&markdown);

        Ok(cleaned_markdown)
    }

    /// Converts preprocessed HTML to markdown using html2text.
    fn html_to_markdown(&self, html: &str) -> Result<String, MarkdownError> {
        let cursor = Cursor::new(html.as_bytes());
        let markdown = from_read(cursor, self.config.max_line_width).map_err(|e| {
            MarkdownError::ParseError {
                message: format!("Failed to convert HTML to markdown: {}", e),
            }
        })?;
        Ok(markdown)
    }

    /// Extracts the title from HTML content.
    fn extract_title(&self, html: &str) -> Option<String> {
        // Simple regex to extract title from HTML
        const TITLE_OPEN_TAG: &str = "<title>";
        if let Some(start) = html.find(TITLE_OPEN_TAG) {
            let title_start = start + TITLE_OPEN_TAG.len();
            if let Some(end) = html[title_start..].find("</title>") {
                let title = &html[title_start..title_start + end];
                return Some(title.trim().to_string());
            }
        }
        None
    }

    /// Generates frontmatter for the markdown document.
    ///
    /// # Arguments
    ///
    /// * `url` - The source URL of the HTML document
    /// * `html_content` - The original HTML content
    /// * `markdown_content` - The converted markdown content
    ///
    /// # Returns
    ///
    /// Returns the markdown content with frontmatter prepended, or a `MarkdownError` on failure.
    fn generate_frontmatter(
        &self,
        url: &str,
        html_content: &str,
        markdown_content: &str,
    ) -> Result<String, MarkdownError> {
        let now = Utc::now();
        let mut builder = FrontmatterBuilder::new(url.to_string())
            .exporter(format!("markdowndown-html-{}", env!("CARGO_PKG_VERSION")))
            .download_date(now);

        // Add default fields
        let mut default_fields = vec![
            ("converted_at", now.to_rfc3339()),
            ("conversion_type", "html".to_string()),
            ("url", url.to_string()),
        ];

        // Try to extract title from HTML
        if let Some(title) = self.extract_title(html_content) {
            default_fields.push(("title", title));
        }

        // Add all default fields
        for (key, value) in default_fields {
            builder = builder.additional_field(key.to_string(), value);
        }

        // Add custom frontmatter fields from configuration
        for (key, value) in &self.output_config.custom_frontmatter_fields {
            builder = builder.additional_field(key.clone(), value.clone());
        }

        let frontmatter = builder.build()?;
        Ok(format!("{frontmatter}\n{markdown_content}"))
    }
}

#[async_trait]
impl Converter for HtmlConverter {
    /// Converts content from a URL to markdown by fetching HTML and converting it.
    async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        // Fetch HTML content from URL with HTML-specific headers
        let headers = std::collections::HashMap::from([(
            "Accept".to_string(),
            "text/html,application/xhtml+xml".to_string(),
        )]);
        let html_content = self.client.get_text_with_headers(url, &headers).await?;

        // Convert HTML to markdown string
        let markdown_string = self.convert_html(&html_content)?;

        // Handle empty content case - provide minimal markdown for empty HTML
        let markdown_content = if markdown_string.trim().is_empty() {
            "<!-- Empty HTML document -->".to_string()
        } else {
            markdown_string
        };

        // Only generate frontmatter if configured to include it
        let final_content = if self.output_config.include_frontmatter {
            self.generate_frontmatter(url, &html_content, &markdown_content)?
        } else {
            markdown_content
        };

        Markdown::new(final_content)
    }

    /// Returns the name of this converter.
    fn name(&self) -> &'static str {
        "HTML"
    }
}

impl Default for HtmlConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, HttpConfig, OutputConfig};
    use std::time::Duration;
    use wiremock::matchers::{method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Test configuration constants
    const DEFAULT_LINE_WIDTH: usize = 120;
    const CUSTOM_LINE_WIDTH: usize = 80;
    const TEST_LINE_WIDTH: usize = 100;
    const NARROW_LINE_WIDTH: usize = 50;
    const HTTP_TIMEOUT_SECS: u64 = 30;
    const HTTP_RETRY_DELAY_SECS: u64 = 1;
    const MAX_RETRY_ATTEMPTS: u32 = 3;
    const MAX_REDIRECTS: u32 = 10;
    const MAX_BLANK_LINES: usize = 3;
    const MAX_CONSECUTIVE_BLANK_LINES: usize = 2;
    const HTTP_STATUS_OK: u16 = 200;

    // Helper function to create a test converter
    fn create_test_converter() -> HtmlConverter {
        HtmlConverter::new()
    }

    // Helper function to setup mock server with HTML content
    async fn setup_mock_server_with_html(path: &str, html: &str) -> (MockServer, String) {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher(path))
            .respond_with(ResponseTemplate::new(HTTP_STATUS_OK).set_body_string(html))
            .mount(&mock_server)
            .await;
        let url = format!("{}{}", mock_server.uri(), path);
        (mock_server, url)
    }

    // Helper function to assert ParseError with expected message fragment
    fn assert_parse_error_contains(result: Result<String, MarkdownError>, expected_fragment: &str) {
        assert!(result.is_err());
        if let Err(MarkdownError::ParseError { message }) = result {
            assert!(message.contains(expected_fragment));
        } else {
            panic!(
                "Expected ParseError with message containing '{}'",
                expected_fragment
            );
        }
    }

    // Helper function to test converter with frontmatter setting
    async fn test_converter_with_frontmatter_setting(
        include_frontmatter: bool,
        html: &str,
        path: &str,
        custom_fields: Vec<(String, String)>,
    ) -> Result<Markdown, MarkdownError> {
        let (_mock_server, url) = setup_mock_server_with_html(path, html).await;
        let output_config = OutputConfig {
            include_frontmatter,
            custom_frontmatter_fields: custom_fields,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(
            HttpClient::new(),
            HtmlConverterConfig::default(),
            output_config,
        );
        converter.convert(&url).await
    }

    #[test]
    fn test_html_converter_new() {
        let converter = create_test_converter();
        assert_eq!(converter.config.max_line_width, DEFAULT_LINE_WIDTH);
        assert!(converter.config.remove_scripts_styles);
    }

    #[test]
    fn test_html_converter_with_config() {
        let config = HtmlConverterConfig {
            max_line_width: CUSTOM_LINE_WIDTH,
            remove_scripts_styles: false,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config_only(config);
        assert_eq!(converter.config.max_line_width, CUSTOM_LINE_WIDTH);
        assert!(!converter.config.remove_scripts_styles);
    }

    #[test]
    fn test_convert_empty_html_error() {
        let converter = create_test_converter();
        let result = converter.convert_html("");
        assert_parse_error_contains(result, "HTML content cannot be empty");
    }

    #[test]
    fn test_convert_whitespace_only_html_error() {
        let converter = create_test_converter();
        let result = converter.convert_html("   \n\t  ");
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_basic_html_success() {
        let converter = create_test_converter();
        let html = "<p>Hello, world!</p>";
        let result = converter.convert_html(html);
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("Hello, world!"));
    }

    #[test]
    fn test_default_implementation() {
        let converter1 = HtmlConverter::new();
        let converter2 = HtmlConverter::default();
        assert_eq!(
            converter1.config.max_line_width,
            converter2.config.max_line_width
        );
    }

    /// Comprehensive tests for improved coverage
    mod comprehensive_coverage_tests {
        use super::*;

        #[test]
        fn test_html_converter_with_full_config() {
            // Test `with_config` method (covers constructor path)
            let http_config = HttpConfig {
                timeout: Duration::from_secs(HTTP_TIMEOUT_SECS),
                user_agent: "test-agent".to_string(),
                max_retries: MAX_RETRY_ATTEMPTS,
                retry_delay: Duration::from_secs(HTTP_RETRY_DELAY_SECS),
                max_redirects: MAX_REDIRECTS,
            };
            let auth_config = AuthConfig {
                github_token: None,
                office365_token: None,
                google_api_key: None,
            };
            let client = HttpClient::with_config(&http_config, &auth_config);

            let html_config = HtmlConverterConfig {
                max_line_width: TEST_LINE_WIDTH,
                remove_scripts_styles: true,
                remove_navigation: false,
                remove_sidebars: true,
                remove_ads: false,
                max_blank_lines: MAX_BLANK_LINES,
            };

            let output_config = OutputConfig {
                include_frontmatter: true,
                custom_frontmatter_fields: vec![(
                    "custom_field".to_string(),
                    "custom_value".to_string(),
                )],
                normalize_whitespace: true,
                max_consecutive_blank_lines: MAX_CONSECUTIVE_BLANK_LINES,
            };

            let converter =
                HtmlConverter::with_config(client, html_config.clone(), output_config.clone());

            assert_eq!(converter.config.max_line_width, TEST_LINE_WIDTH);
            assert!(!converter.config.remove_navigation);
            assert!(!converter.config.remove_ads);
            assert_eq!(converter.config.max_blank_lines, MAX_BLANK_LINES);
            assert_eq!(converter.output_config.custom_frontmatter_fields.len(), 1);
        }

        #[test]
        fn test_extract_title_with_title_tag() {
            let converter = create_test_converter();
            let html = "<html><head><title>Test Page Title</title></head><body><p>Content</p></body></html>";

            let title = converter.extract_title(html);
            assert!(title.is_some());
            assert_eq!(title.unwrap(), "Test Page Title");
        }

        #[test]
        fn test_extract_title_no_title_tag() {
            let converter = create_test_converter();
            let html = "<html><head></head><body><p>Content without title</p></body></html>";

            let title = converter.extract_title(html);
            assert!(title.is_none());
        }

        #[test]
        fn test_extract_title_malformed_tag() {
            let converter = create_test_converter();
            let html = "<html><head><title>Incomplete title tag";

            let title = converter.extract_title(html);
            assert!(title.is_none());
        }

        #[test]
        fn test_extract_title_with_whitespace() {
            let converter = create_test_converter();
            let html = "<title>   Trimmed Title   </title>";

            let title = converter.extract_title(html);
            assert!(title.is_some());
            assert_eq!(title.unwrap(), "Trimmed Title");
        }

        #[tokio::test]
        async fn test_converter_async_with_frontmatter() {
            // Test the async convert method with frontmatter enabled
            let html_content = r#"<html><head><title>Test Document</title></head><body><h1>Main Heading</h1><p>This is test content.</p></body></html>"#;

            let custom_fields = vec![
                ("author".to_string(), "test-author".to_string()),
                ("category".to_string(), "test-category".to_string()),
            ];

            let result = test_converter_with_frontmatter_setting(
                true,
                html_content,
                "/test-page",
                custom_fields,
            )
            .await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            let content = markdown.as_str();

            // Should have frontmatter
            assert!(content.starts_with("---"));
            assert!(content.contains("title: Test Document"));
            assert!(content.contains("author: test-author"));
            assert!(content.contains("category: test-category"));
            assert!(content.contains("converted_at:"));
            assert!(content.contains("conversion_type: html"));

            // Should have converted content
            assert!(content.contains("# Main Heading"));
            assert!(content.contains("This is test content."));
        }

        #[tokio::test]
        async fn test_converter_async_without_frontmatter() {
            // Test the async convert method with frontmatter disabled
            let html_content = "<h1>Simple Test</h1><p>Basic content.</p>";

            let result = test_converter_with_frontmatter_setting(
                false,
                html_content,
                "/simple-page",
                vec![],
            )
            .await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            let content = markdown.as_str();

            // Should NOT have frontmatter
            assert!(!content.starts_with("---"));
            assert!(!content.contains("title:"));
            assert!(!content.contains("converted_at:"));

            // Should have converted content
            assert!(content.contains("# Simple Test"));
            assert!(content.contains("Basic content."));
        }

        #[tokio::test]
        async fn test_converter_async_empty_html_response() {
            // Test handling of empty HTML response from server
            let (_mock_server, url) = setup_mock_server_with_html("/empty-page", "").await;

            let converter = create_test_converter();
            let result = converter.convert(&url).await;

            // Should fail because empty HTML content is invalid
            assert!(result.is_err());
            match result.unwrap_err() {
                MarkdownError::ParseError { message } => {
                    assert!(message.contains("HTML content cannot be empty"));
                }
                other_error => {
                    panic!("Expected ParseError for empty HTML, but got: {other_error:?}");
                }
            }
        }

        #[tokio::test]
        async fn test_converter_async_whitespace_html_to_minimal_content() {
            // Test handling of mostly empty HTML that results in empty markdown
            let minimal_html = "<html><body>  </body></html>";

            let (_mock_server, url) =
                setup_mock_server_with_html("/minimal-page", minimal_html).await;

            let converter = create_test_converter();
            let result = converter.convert(&url).await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            let content = markdown.as_str();

            // Should contain the empty document comment when markdown is empty
            assert!(content.contains("<!-- Empty HTML document -->"));
        }

        #[test]
        fn test_converter_name() {
            let converter = create_test_converter();
            assert_eq!(converter.name(), "HTML");
        }

        #[test]
        fn test_html_to_markdown_direct() {
            // Test the html_to_markdown method directly
            let converter = create_test_converter();
            let html = "<h1>Direct Test</h1><p>Testing html_to_markdown method.</p>";

            let result = converter.html_to_markdown(html);
            assert!(result.is_ok());

            let markdown = result.unwrap();
            assert!(markdown.contains("Direct Test"));
            assert!(markdown.contains("Testing html_to_markdown method"));
        }

        #[tokio::test]
        async fn test_converter_async_no_title_tag() {
            // Test async conversion with HTML that has no title tag
            let html_content = "<h1>No Title Tag</h1><p>Content without title tag.</p>";

            let (_mock_server, url) = setup_mock_server_with_html("/no-title", html_content).await;

            // Create converter with frontmatter enabled to test title extraction path
            let output_config = OutputConfig {
                include_frontmatter: true,
                ..Default::default()
            };

            let converter = HtmlConverter::with_config(
                HttpClient::new(),
                HtmlConverterConfig::default(),
                output_config,
            );

            let result = converter.convert(&url).await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            let content = markdown.as_str();

            // Should have frontmatter but no title field since no title tag was found
            assert!(content.starts_with("---"));
            assert!(!content.contains("title:"));
            assert!(content.contains("converted_at:"));
            assert!(content.contains("conversion_type: html"));
        }

        #[test]
        fn test_convert_html_with_custom_line_width() {
            // Test HTML conversion with custom line width configuration
            let config = HtmlConverterConfig {
                max_line_width: NARROW_LINE_WIDTH,
                ..Default::default()
            };

            let converter = HtmlConverter::with_config_only(config);
            let html = "<p>This is a very long paragraph that should be wrapped according to the custom line width setting that we have configured for this test.</p>";

            let result = converter.convert_html(html);
            assert!(result.is_ok());

            let markdown = result.unwrap();
            // The exact wrapping behavior depends on html2text implementation,
            // but we can verify the conversion succeeded
            assert!(markdown.contains("very long paragraph"));
        }
    }
}
