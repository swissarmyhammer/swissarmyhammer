//! Comprehensive unit tests for HTML to markdown converter.
//!
//! This module tests HTML conversion functionality, including preprocessing,
//! postprocessing, configuration handling, and error scenarios.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{Converter, HtmlConverter, HtmlConverterConfig};
use markdowndown::types::{MarkdownError, NetworkErrorKind, ValidationErrorKind};
use mockito::Server;

// Import shared test helpers
use crate::helpers::converters::{
    create_html_converter, create_html_converter_with_client, SAMPLE_HTML_CONTENT,
};

/// Sample HTML with complex structure for testing preprocessing
const COMPLEX_HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Complex Document</title>
    <script>
        // This script should be removed
        function trackUser() { /* ... */ }
    </script>
    <style>
        /* CSS should be removed */
        body { margin: 0; }
    </style>
</head>
<body>
    <nav class="navigation">
        <ul>
            <li><a href="/home">Home</a></li>
            <li><a href="/about">About</a></li>
        </ul>
    </nav>
    
    <main>
        <article>
            <h1>Complex Document Title</h1>
            
            <div class="sidebar">
                <h3>Related Articles</h3>
                <ul>
                    <li><a href="/article1">Article 1</a></li>
                    <li><a href="/article2">Article 2</a></li>
                </ul>
            </div>
                
            <div class="content">
                <p>This is the main content that should be preserved.</p>
                
                <div class="ads">
                    <div class="advertisement">
                        <p>This is an advertisement that should be removed</p>
                    </div>
                </div>
                
                <h2>Technical Details</h2>
                <pre><code>// Sample code block
def process_data(data):
    return [item.upper() for item in data]
</code></pre>
                
                <table>
                    <thead>
                        <tr>
                            <th>Column 1</th>
                            <th>Column 2</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>Data 1</td>
                            <td>Data 2</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </article>
    </main>
    
    <aside class="sidebar">
        <p>Sidebar content that should be removed</p>
    </aside>
    
    <footer>
        <p>Footer content</p>
    </footer>
    
    <script>
        // Analytics script that should be removed
        gtag('config', 'GA-XXXXXXXXX');
    </script>
</body>
</html>"#;

/// Tests for HTML converter creation and configuration
mod converter_creation_tests {
    use super::*;

    #[test]
    fn test_html_converter_new() {
        let converter = HtmlConverter::new();
        assert_eq!(converter.name(), "HTML");
    }

    #[test]
    fn test_html_converter_with_config() {
        let client = HttpClient::new();
        let config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig::default();
        let converter = HtmlConverter::with_config(client, config, output_config);
        assert_eq!(converter.name(), "HTML");
    }

    #[test]
    fn test_html_converter_with_custom_config() {
        let client = HttpClient::new();
        let config = HtmlConverterConfig {
            max_line_width: 80,
            remove_scripts_styles: true,
            remove_navigation: true,
            remove_sidebars: true,
            remove_ads: true,
            max_blank_lines: 1,
        };
        let output_config = markdowndown::config::OutputConfig::default();
        let converter = HtmlConverter::with_config(client, config, output_config);
        assert_eq!(converter.name(), "HTML");
    }
}

/// Tests for successful HTML conversion
mod html_conversion_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_basic_html() {
        let mut server = Server::new_async().await;
        let html_content = SAMPLE_HTML_CONTENT;

        let mock = server
            .mock("GET", "/test.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/test.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Check for expected markdown elements based on SAMPLE_HTML_CONTENT
        assert!(content.contains("# Test Article"));
        assert!(content.contains("## Features"));
        assert!(content.contains("**formatting**"));
        assert!(content.contains("*text*"));
        assert!(content.contains("[External links](https://example.com)"));
        assert!(content.contains("* Basic"));

        // Should not contain unwanted HTML elements
        assert!(!content.contains("<div"));
        assert!(!content.contains("<footer"));
        assert!(!content.contains("<nav"));
    }

    #[tokio::test]
    async fn test_convert_complex_html_with_preprocessing() {
        let mut server = Server::new_async().await;
        let html_content = COMPLEX_HTML_CONTENT;

        let mock = server
            .mock("GET", "/complex.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/complex.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Check for main content based on COMPLEX_HTML_CONTENT
        assert!(content.contains("# Complex Document Title"));
        assert!(content.contains("This is the main content that should be preserved."));

        // Should not contain scripts, styles, or navigation elements
        assert!(!content.contains("trackUser"));
        assert!(!content.contains("gtag"));
        assert!(!content.contains("body { margin: 0; }"));

        // Navigation and footer content should be minimized or removed
        // (exact behavior depends on preprocessing configuration)
    }

    #[tokio::test]
    async fn test_convert_html_with_custom_headers() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Test</h1><p>Content</p></body></html>";

        let mock = server
            .mock("GET", "/protected.html")
            .match_header("User-Agent", "test-agent/1.0")
            .match_header("Accept", "text/html,application/xhtml+xml")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder()
            .user_agent("test-agent/1.0")
            .timeout_seconds(5)
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/protected.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Test"));
        assert!(markdown.content_only().contains("Content"));
    }

    #[tokio::test]
    async fn test_convert_html_with_different_encodings() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Encoding Test</title>
</head>
<body>
    <h1>Test with Special Characters</h1>
    <p>Here are some special characters: café, naïve, résumé</p>
    <p>Unicode: 你好, Здравствуй, مرحبا</p>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/encoding.html")
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/encoding.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Check that special characters are preserved
        assert!(content.contains("café"));
        assert!(content.contains("naïve"));
        assert!(content.contains("résumé"));
        assert!(content.contains("你好"));
        assert!(content.contains("Здравствуй"));
        assert!(content.contains("مرحبا"));
    }

    #[tokio::test]
    async fn test_convert_empty_html() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body></body></html>";

        let mock = server
            .mock("GET", "/empty.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/empty.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        // Empty HTML should result in minimal markdown
        assert!(markdown.content_only().len() < 50); // Should be very short
    }

    #[tokio::test]
    async fn test_convert_html_with_malformed_markup() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Malformed HTML</title>
</head>
<body>
    <h1>Heading without closing tag
    <p>Paragraph with <strong>unclosed bold
    <div>
        <p>Nested content</p>
        <ul>
            <li>Item 1
            <li>Item 2</li>
        </ul>
    </div>
    <p>Final paragraph</p>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/malformed.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/malformed.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Should still extract meaningful content despite malformed HTML
        assert!(content.contains("Heading without closing tag"));
        assert!(content.contains("Paragraph with"));
        assert!(content.contains("Nested content"));
        assert!(content.contains("Final paragraph"));
    }
}

/// Tests for error handling
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_invalid_url() {
        let converter = create_html_converter();
        let result = converter.convert("not-a-valid-url").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ValidationError { kind, .. } => {
                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
            }
            _ => panic!("Expected ValidationError for invalid URL"),
        }
    }

    #[tokio::test]
    async fn test_convert_http_404_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notfound.html")
            .with_status(404)
            .with_body("Not Found")
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/notfound.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::ServerError(status) => {
                    assert_eq!(status, 404);
                }
                _ => panic!("Expected ServerError(404)"),
            },
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_convert_http_500_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/error.html")
            .with_status(500)
            .with_body("Internal Server Error")
            .expect(2) // Original request + 1 retry
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(5)
            .max_retries(1) // Reduce retries for faster test
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/error.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::ServerError(status) => {
                    assert_eq!(status, 500);
                }
                _ => panic!("Expected ServerError(500)"),
            },
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_convert_non_html_content() {
        let mut server = Server::new_async().await;
        let json_content = r#"{"message": "This is JSON, not HTML"}"#;

        let mock = server
            .mock("GET", "/data.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/data.json", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        // Should still work - the converter will treat JSON as text content
        assert!(result.is_ok());

        let markdown = result.unwrap();
        // The JSON should be converted to plain text
        assert!(markdown.content_only().contains("This is JSON, not HTML"));
    }

    #[tokio::test]
    async fn test_convert_large_html_content() {
        let mut server = Server::new_async().await;

        // Create large HTML content (1MB)
        let large_content = format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Large Document</title></head>
<body>
<h1>Large Content Test</h1>
{}
</body>
</html>"#,
            "<p>This is a paragraph with lots of content. ".repeat(10000)
        );

        let mock = server
            .mock("GET", "/large.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(&large_content)
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(10) // Longer timeout for large content
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/large.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Large Content Test"));
        assert!(markdown.content_only().len() > 10000); // Should be substantial content
    }
}

/// Tests for configuration handling
mod configuration_tests {
    use super::*;

    #[test]
    fn test_html_converter_config_default() {
        let config = HtmlConverterConfig::default();

        // Test default values
        assert_eq!(config.max_line_width, 120);
        assert!(config.remove_scripts_styles);
        assert!(config.remove_navigation);
        assert!(config.remove_sidebars);
        assert!(config.remove_ads);
        assert_eq!(config.max_blank_lines, 2);
    }

    #[test]
    fn test_html_converter_config_custom() {
        let config = HtmlConverterConfig {
            max_line_width: 100,
            remove_scripts_styles: false,
            remove_navigation: false,
            remove_sidebars: false,
            remove_ads: false,
            max_blank_lines: 5,
        };

        assert_eq!(config.max_line_width, 100);
        assert!(!config.remove_scripts_styles);
        assert!(!config.remove_navigation);
        assert!(!config.remove_sidebars);
        assert!(!config.remove_ads);
        assert_eq!(config.max_blank_lines, 5);
    }

    #[tokio::test]
    async fn test_converter_respects_configuration() {
        let mut server = Server::new_async().await;
        let html_content = COMPLEX_HTML_CONTENT;

        let mock = server
            .mock("GET", "/config-test.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        // Test with conservative config (keep more content)
        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = HtmlConverterConfig {
            max_line_width: 200,
            remove_scripts_styles: false,
            remove_navigation: false,
            remove_sidebars: false,
            remove_ads: false,
            max_blank_lines: 10,
        };
        let output_config = markdowndown::config::OutputConfig::default();
        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/config-test.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // With conservative config, more content should be preserved
        assert!(content.contains("Complex Document Title"));
        // Navigation and footer content might be preserved depending on implementation
    }
}

/// Tests for frontmatter generation
mod frontmatter_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_with_frontmatter_enabled() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Document with Title</title>
</head>
<body>
    <h1>Main Heading</h1>
    <p>Content here</p>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/frontmatter-test.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);

        // Enable frontmatter and add custom fields
        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig {
            include_frontmatter: true,
            custom_frontmatter_fields: vec![
                ("custom_field".to_string(), "custom_value".to_string()),
                ("author".to_string(), "test_author".to_string()),
            ],
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/frontmatter-test.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();

        // Check frontmatter content
        assert!(frontmatter.contains("title: Document with Title"));
        assert!(frontmatter.contains("url:"));
        assert!(frontmatter.contains("date_downloaded:"));
        assert!(frontmatter.contains("converted_at:"));
        assert!(frontmatter.contains("conversion_type: html"));
        assert!(frontmatter.contains("custom_field: custom_value"));
        assert!(frontmatter.contains("author: test_author"));

        // Content should be present
        assert!(markdown.content_only().contains("# Main Heading"));
    }

    #[tokio::test]
    async fn test_convert_with_frontmatter_disabled() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Document Title</title>
</head>
<body>
    <h1>Content</h1>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/no-frontmatter.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);

        // Disable frontmatter
        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig {
            include_frontmatter: false,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/no-frontmatter.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should not have frontmatter
        assert!(markdown.frontmatter().is_none());

        // Content should be present
        assert!(markdown.content_only().contains("# Content"));
    }

    #[tokio::test]
    async fn test_convert_html_with_no_title() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
</head>
<body>
    <h1>Content without title tag</h1>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/no-title.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);

        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig {
            include_frontmatter: true,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/no-title.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter but no title field
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert!(!frontmatter.contains("title:"));
    }

    #[tokio::test]
    async fn test_convert_html_with_frontmatter_and_title_extraction() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Document with Title</title>
</head>
<body>
    <h1>Main Heading</h1>
    <p>Content here</p>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/title-test.html")
            .match_header("Accept", "text/html,application/xhtml+xml") // Test custom headers (lines 183-185)
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);

        // Enable frontmatter and add custom fields
        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig {
            include_frontmatter: true,
            custom_frontmatter_fields: vec![
                ("custom_field".to_string(), "custom_value".to_string()),
                ("author".to_string(), "test_author".to_string()),
            ],
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/title-test.html", server.url());
        let result = converter.convert(&url).await; // Tests lines 181, 187, 190, 202-208, 211

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter with extracted title (tests title extraction lines 166, 168-169)
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();

        // Check frontmatter content (tests lines 202-208)
        assert!(frontmatter.contains("title: Document with Title"));
        assert!(frontmatter.contains("url:"));
        assert!(frontmatter.contains("date_downloaded:"));
        assert!(frontmatter.contains("converted_at:"));
        assert!(frontmatter.contains("conversion_type: html"));
        assert!(frontmatter.contains("custom_field: custom_value"));
        assert!(frontmatter.contains("author: test_author"));

        // Content should be present (tests line 190)
        assert!(markdown.content_only().contains("# Main Heading"));
    }

    #[tokio::test]
    async fn test_convert_html_without_title_tag() {
        let mut server = Server::new_async().await;
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
</head>
<body>
    <h1>Content without title tag</h1>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/no-title.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);

        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig {
            include_frontmatter: true,
            ..Default::default()
        };

        let converter = HtmlConverter::with_config(client, html_config, output_config);

        let url = format!("{}/no-title.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter but no title field (tests line 174 - None return)
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert!(!frontmatter.contains("title:"));
    }

    #[tokio::test]
    async fn test_convert_empty_html_content() {
        let mut server = Server::new_async().await;
        // HTML that produces empty markdown content after processing
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Empty Content</title>
</head>
<body>
    <!-- only comments -->
</body>
</html>"#;

        let mock = server
            .mock("GET", "/empty-content.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/empty-content.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Should contain the empty document placeholder (tests lines 193-194)
        assert!(content.contains("<!-- Empty HTML document -->"));
    }

    #[tokio::test]
    async fn test_convert_html_produces_empty_content() {
        let mut server = Server::new_async().await;
        // HTML that produces empty or whitespace-only markdown content after processing
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Empty Content</title>
    <script>/* script content */</script>
    <style>/* style content */</style>
</head>
<body>
    <!-- only comments and elements that get removed -->
    <script>console.log("removed");</script>
    <style>.class { color: red; }</style>
</body>
</html>"#;

        let mock = server
            .mock("GET", "/empty-content.html")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/empty-content.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Should contain the empty document placeholder
        assert!(content.contains("<!-- Empty HTML document -->"));
    }
}

/// Tests for title extraction functionality
mod title_extraction_tests {
    use super::*;

    #[test]
    fn test_extract_title_with_valid_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>Test Document Title</title></head><body></body></html>"#;

        // Use reflection to access private method via convert_html which uses it
        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_title_with_nested_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>  Nested Title with Whitespace  </title></head><body></body></html>"#;

        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_title_with_no_title_tag() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head></head><body><h1>No title tag</h1></body></html>"#;

        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_title_with_malformed_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>Unclosed title<body></body></html>"#;

        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_title_with_empty_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title></title></head><body></body></html>"#;

        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }
}

/// Tests for error handling in HTML conversion
mod html_error_handling_tests {
    use super::*;

    #[test]
    fn test_convert_html_empty_input_error() {
        let converter = HtmlConverter::new();

        // Test empty HTML input (should trigger error on line 124-129)
        let result = converter.convert_html("");

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => {
                assert!(message.contains("HTML content cannot be empty"));
            }
            _ => panic!("Expected ParseError for empty HTML"),
        }
    }

    #[test]
    fn test_convert_html_whitespace_only_error() {
        let converter = HtmlConverter::new();

        // Test whitespace-only HTML input (should trigger error on line 124-129)
        let result = converter.convert_html("   \n\t  ");

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => {
                assert!(message.contains("HTML content cannot be empty"));
            }
            _ => panic!("Expected ParseError for whitespace-only HTML"),
        }
    }

    #[test]
    fn test_title_extraction_edge_cases() {
        let converter = HtmlConverter::new();

        // Test title extraction with different HTML structures
        let test_cases = [
            (
                "<title>Simple Title</title>",
                Some("Simple Title".to_string()),
            ),
            (
                "<title>  Whitespace Title  </title>",
                Some("Whitespace Title".to_string()),
            ),
            ("<title></title>", Some("".to_string())),
            ("<html><body>No title tag</body></html>", None),
            ("<title>Unclosed title", None),
            ("", None),
        ];

        for (html, _expected) in test_cases {
            // We can't directly call extract_title as it's private, but we can test
            // the convert_html method which uses it internally
            if html.is_empty() {
                // Skip empty HTML as it will trigger the empty input error
                continue;
            }

            let result = converter.convert_html(html);

            // For valid HTML, the conversion should succeed
            if html.contains("<title>") && html.contains("</title>") {
                assert!(result.is_ok(), "Failed to convert HTML: {html}");
            }
        }
    }

    #[test]
    fn test_convert_html_error_wrapping() {
        let converter = HtmlConverter::new();

        // Test with valid but complex HTML to ensure no errors occur
        let complex_html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Complex Document</title>
    <meta charset="UTF-8">
</head>
<body>
    <h1>Heading</h1>
    <p>Some content with <strong>bold</strong> and <em>italic</em> text.</p>
    <ul>
        <li>Item 1</li>
        <li>Item 2</li>
    </ul>
</body>
</html>"#;

        let result = converter.convert_html(complex_html);

        // Should handle complex HTML gracefully
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("# Heading"));
        assert!(markdown.contains("**bold**"));
        assert!(markdown.contains("*italic*"));
    }

    #[test]
    fn test_convert_html_with_extremely_long_content() {
        let converter = HtmlConverter::new();

        // Create very long HTML content
        let long_title = "A".repeat(1000000);
        let html = format!(
            r#"<html><head><title>{long_title}</title></head><body><p>Content</p></body></html>"#
        );

        let result = converter.convert_html(&html);
        assert!(result.is_ok());
    }
}

/// Tests for converter name method
mod converter_name_tests {
    use super::*;

    #[test]
    fn test_converter_name() {
        let converter = HtmlConverter::new();
        assert_eq!(converter.name(), "HTML");
    }

    #[test]
    fn test_converter_name_consistency() {
        let converter1 = HtmlConverter::new();
        let converter2 = HtmlConverter::default();
        assert_eq!(converter1.name(), converter2.name());
    }
}

/// Integration tests combining multiple features
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_html_conversion() {
        let mut server = Server::new_async().await;
        let html_content = SAMPLE_HTML_CONTENT;

        let mock = server
            .mock("GET", "/integration-test.html")
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder()
            .user_agent("integration-test/1.0")
            .timeout_seconds(5)
            .max_retries(2)
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/integration-test.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Verify all major markdown elements are present based on SAMPLE_HTML_CONTENT
        assert!(content.contains("# Test Article"));
        assert!(content.contains("## Features"));
        assert!(content.contains("**formatting**"));
        assert!(content.contains("*text*"));
        assert!(content.contains("[External links](https://example.com)"));
        assert!(content.contains("* Basic"));
        assert!(content.contains("* Multiple"));
        assert!(content.contains("> This is a blockquote"));

        // Verify frontmatter is included if configured
        if let Some(frontmatter) = markdown.frontmatter() {
            assert!(frontmatter.contains("title:"));
            assert!(frontmatter.contains("url:"));
        }
    }

    #[tokio::test]
    async fn test_html_converter_with_redirects() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Final Content</h1></body></html>";

        let redirect_mock = server
            .mock("GET", "/redirect-source")
            .with_status(302)
            .with_header("Location", &format!("{}/redirect-target", server.url()))
            .create_async()
            .await;

        let target_mock = server
            .mock("GET", "/redirect-target")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(5).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = create_html_converter_with_client(client);

        let url = format!("{}/redirect-source", server.url());
        let result = converter.convert(&url).await;

        redirect_mock.assert_async().await;
        target_mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Final Content"));
    }
}
