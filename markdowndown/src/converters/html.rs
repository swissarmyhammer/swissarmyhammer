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
        if let Some(start) = html.find("<title>") {
            if let Some(end) = html[start + 7..].find("</title>") {
                let title = &html[start + 7..start + 7 + end];
                return Some(title.trim().to_string());
            }
        }
        None
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
        if self.output_config.include_frontmatter {
            // Generate frontmatter
            let now = Utc::now();
            let mut builder = FrontmatterBuilder::new(url.to_string())
                .exporter(format!("markdowndown-html-{}", env!("CARGO_PKG_VERSION")))
                .download_date(now)
                .additional_field("converted_at".to_string(), now.to_rfc3339())
                .additional_field("conversion_type".to_string(), "html".to_string())
                .additional_field("url".to_string(), url.to_string());

            // Try to extract title from HTML
            if let Some(title) = self.extract_title(&html_content) {
                builder = builder.additional_field("title".to_string(), title);
            }

            // Add custom frontmatter fields from configuration
            for (key, value) in &self.output_config.custom_frontmatter_fields {
                builder = builder.additional_field(key.clone(), value.clone());
            }

            let frontmatter = builder.build()?;

            // Combine frontmatter with content
            let markdown_with_frontmatter = format!("{frontmatter}\n{markdown_content}");

            // Wrap in Markdown type with validation
            Markdown::new(markdown_with_frontmatter)
        } else {
            // No frontmatter - just return the markdown content
            Markdown::new(markdown_content)
        }
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_html_converter_new() {
        let converter = HtmlConverter::new();
        assert_eq!(converter.config.max_line_width, 120);
        assert!(converter.config.remove_scripts_styles);
    }

    #[test]
    fn test_html_converter_with_config() {
        let config = HtmlConverterConfig {
            max_line_width: 80,
            remove_scripts_styles: false,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config_only(config);
        assert_eq!(converter.config.max_line_width, 80);
        assert!(!converter.config.remove_scripts_styles);
    }

    #[test]
    fn test_convert_empty_html_error() {
        let converter = HtmlConverter::new();
        let result = converter.convert_html("");
        assert!(result.is_err());

        if let Err(MarkdownError::ParseError { message }) = result {
            assert!(message.contains("HTML content cannot be empty"));
        } else {
            panic!("Expected ParseError with specific message");
        }
    }

    #[test]
    fn test_convert_whitespace_only_html_error() {
        let converter = HtmlConverter::new();
        let result = converter.convert_html("   \n\t  ");
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_basic_html_success() {
        let converter = HtmlConverter::new();
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
                timeout: Duration::from_secs(30),
                user_agent: "test-agent".to_string(),
                max_retries: 3,
                retry_delay: Duration::from_secs(1),
                max_redirects: 10,
            };
            let auth_config = AuthConfig {
                github_token: None,
                office365_token: None,
                google_api_key: None,
            };
            let client = HttpClient::with_config(&http_config, &auth_config);

            let html_config = HtmlConverterConfig {
                max_line_width: 100,
                remove_scripts_styles: true,
                remove_navigation: false,
                remove_sidebars: true,
                remove_ads: false,
                max_blank_lines: 3,
            };

            let output_config = OutputConfig {
                include_frontmatter: true,
                custom_frontmatter_fields: vec![(
                    "custom_field".to_string(),
                    "custom_value".to_string(),
                )],
                normalize_whitespace: true,
                max_consecutive_blank_lines: 2,
            };

            let converter =
                HtmlConverter::with_config(client, html_config.clone(), output_config.clone());

            assert_eq!(converter.config.max_line_width, 100);
            assert!(!converter.config.remove_navigation);
            assert!(!converter.config.remove_ads);
            assert_eq!(converter.config.max_blank_lines, 3);
            assert_eq!(converter.output_config.custom_frontmatter_fields.len(), 1);
        }

        #[test]
        fn test_extract_title_with_title_tag() {
            let converter = HtmlConverter::new();
            let html = "<html><head><title>Test Page Title</title></head><body><p>Content</p></body></html>";

            let title = converter.extract_title(html);
            assert!(title.is_some());
            assert_eq!(title.unwrap(), "Test Page Title");
        }

        #[test]
        fn test_extract_title_no_title_tag() {
            let converter = HtmlConverter::new();
            let html = "<html><head></head><body><p>Content without title</p></body></html>";

            let title = converter.extract_title(html);
            assert!(title.is_none());
        }

        #[test]
        fn test_extract_title_malformed_tag() {
            let converter = HtmlConverter::new();
            let html = "<html><head><title>Incomplete title tag";

            let title = converter.extract_title(html);
            assert!(title.is_none());
        }

        #[test]
        fn test_extract_title_with_whitespace() {
            let converter = HtmlConverter::new();
            let html = "<title>   Trimmed Title   </title>";

            let title = converter.extract_title(html);
            assert!(title.is_some());
            assert_eq!(title.unwrap(), "Trimmed Title");
        }

        #[tokio::test]
        async fn test_converter_async_with_frontmatter() {
            // Test the async convert method with frontmatter enabled
            let mock_server = MockServer::start().await;

            let html_content = r#"<html><head><title>Test Document</title></head><body><h1>Main Heading</h1><p>This is test content.</p></body></html>"#;

            Mock::given(method("GET"))
                .and(path("/test-page"))
                .respond_with(ResponseTemplate::new(200).set_body_string(html_content))
                .mount(&mock_server)
                .await;

            // Create converter with frontmatter enabled
            let mut output_config = OutputConfig::default();
            output_config.include_frontmatter = true;
            output_config.custom_frontmatter_fields = vec![
                ("author".to_string(), "test-author".to_string()),
                ("category".to_string(), "test-category".to_string()),
            ];

            let converter = HtmlConverter::with_config(
                HttpClient::new(),
                HtmlConverterConfig::default(),
                output_config,
            );

            let url = format!("{}/test-page", mock_server.uri());
            let result = converter.convert(&url).await;

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
            let mock_server = MockServer::start().await;

            let html_content = "<h1>Simple Test</h1><p>Basic content.</p>";

            Mock::given(method("GET"))
                .and(path("/simple-page"))
                .respond_with(ResponseTemplate::new(200).set_body_string(html_content))
                .mount(&mock_server)
                .await;

            // Create converter with frontmatter disabled
            let mut output_config = OutputConfig::default();
            output_config.include_frontmatter = false;

            let converter = HtmlConverter::with_config(
                HttpClient::new(),
                HtmlConverterConfig::default(),
                output_config,
            );

            let url = format!("{}/simple-page", mock_server.uri());
            let result = converter.convert(&url).await;

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
            let mock_server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/empty-page"))
                .respond_with(ResponseTemplate::new(200).set_body_string(""))
                .mount(&mock_server)
                .await;

            let converter = HtmlConverter::new();
            let url = format!("{}/empty-page", mock_server.uri());
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
            let mock_server = MockServer::start().await;

            let minimal_html = "<html><body>  </body></html>";

            Mock::given(method("GET"))
                .and(path("/minimal-page"))
                .respond_with(ResponseTemplate::new(200).set_body_string(minimal_html))
                .mount(&mock_server)
                .await;

            let converter = HtmlConverter::new();
            let url = format!("{}/minimal-page", mock_server.uri());
            let result = converter.convert(&url).await;

            assert!(result.is_ok());
            let markdown = result.unwrap();
            let content = markdown.as_str();

            // Should contain the empty document comment when markdown is empty
            assert!(content.contains("<!-- Empty HTML document -->"));
        }

        #[test]
        fn test_converter_name() {
            let converter = HtmlConverter::new();
            assert_eq!(converter.name(), "HTML");
        }

        #[test]
        fn test_html_to_markdown_direct() {
            // Test the html_to_markdown method directly
            let converter = HtmlConverter::new();
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
            let mock_server = MockServer::start().await;

            let html_content = "<h1>No Title Tag</h1><p>Content without title tag.</p>";

            Mock::given(method("GET"))
                .and(path("/no-title"))
                .respond_with(ResponseTemplate::new(200).set_body_string(html_content))
                .mount(&mock_server)
                .await;

            // Create converter with frontmatter enabled to test title extraction path
            let mut output_config = OutputConfig::default();
            output_config.include_frontmatter = true;

            let converter = HtmlConverter::with_config(
                HttpClient::new(),
                HtmlConverterConfig::default(),
                output_config,
            );

            let url = format!("{}/no-title", mock_server.uri());
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
                max_line_width: 50,
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
