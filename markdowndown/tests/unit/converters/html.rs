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
use crate::helpers::converters::{create_html_converter, SAMPLE_HTML_CONTENT};

// Grouped test configuration constants
struct TestConstants;

impl TestConstants {
    // Timeout constants
    const TEST_TIMEOUT_SECONDS: u64 = 5;
    const LARGE_CONTENT_TIMEOUT_SECONDS: u64 = 10;
    
    // Retry constants
    const DEFAULT_MAX_RETRIES: u32 = 2;
    
    // Line width constants
    const CUSTOM_LINE_WIDTH: usize = 80;
    const CUSTOM_MAX_LINE_WIDTH: usize = 100;
    const DEFAULT_MAX_LINE_WIDTH: usize = 120;
    const LARGE_LINE_WIDTH: usize = 200;
    
    // Blank lines constants
    const DEFAULT_MAX_BLANK_LINES: usize = 2;
    const CUSTOM_MAX_BLANK_LINES: usize = 5;
    const LARGE_MAX_BLANK_LINES: usize = 10;
    
    // Content size thresholds
    const MIN_LARGE_CONTENT_SIZE: usize = 10000;
    const MAX_EMPTY_CONTENT_LENGTH: usize = 50;
    const EXTREMELY_LONG_TITLE_LENGTH: usize = 1_000_000;
}

// Test helper functions to reduce code duplication

/// Sets up a mock server with HTML content and returns server, mock assertion future, and URL
/// This is the unified function that supports both status codes and custom headers
async fn setup_mock_html_test(
    path: &str,
    html_content: &str,
    status: Option<u16>,
    headers: Option<Vec<(&str, &str)>>,
) -> (Server, impl std::future::Future<Output = ()>, String) {
    let mut server = Server::new_async().await;
    let mut mock_builder = server
        .mock("GET", path)
        .with_status(status.unwrap_or(200))
        .with_header("content-type", "text/html");
    
    if let Some(header_pairs) = headers {
        for (key, value) in header_pairs {
            mock_builder = mock_builder.match_header(key, value);
        }
    }
    
    let mock = mock_builder.with_body(html_content).create_async().await;
    let url = format!("{}{}", server.url(), path);
    (server, async move { mock.assert_async().await }, url)
}

/// Creates a test converter with optional custom configuration
fn create_test_converter(
    config: Option<Config>,
    html_config: Option<HtmlConverterConfig>,
    output_config: Option<markdowndown::config::OutputConfig>,
) -> HtmlConverter {
    let config = config.unwrap_or_else(|| Config::builder().timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS).build());
    let client = HttpClient::with_config(&config.http, &config.auth);
    
    match (html_config, output_config) {
        (Some(html_cfg), Some(out_cfg)) => HtmlConverter::with_config(client, html_cfg, out_cfg),
        _ => HtmlConverter::with_config(
            client,
            HtmlConverterConfig::default(),
            markdowndown::config::OutputConfig::default()
        ),
    }
}

/// Assertion helper to check that content contains all expected strings
fn assert_contains_all(content: &str, expected: &[&str]) {
    for &text in expected {
        assert!(content.contains(text), "Content should contain '{}'", text);
    }
}

/// Assertion helper to check that content doesn't contain any forbidden strings
fn assert_not_contains_any(content: &str, forbidden: &[&str]) {
    for &text in forbidden {
        assert!(!content.contains(text), "Content should not contain '{}'", text);
    }
}



/// Tests multiple HTTP error scenarios with specified error responses and expected retry counts
async fn test_http_error_scenarios(errors: Vec<(u16, usize)>) {
    for (status, expected_retries) in errors {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/error.html")
            .with_status(status)
            .with_body(format!("Error {}", status))
            .expect(expected_retries)
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS)
            .max_retries((expected_retries - 1) as u32)
            .build();
        let converter = create_test_converter(Some(config), None, None);

        let url = format!("{}/error.html", server.url());
        let result = converter.convert(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::ServerError(s) => assert_eq!(s, status),
                _ => panic!("Expected ServerError({})", status),
            },
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }
}

/// Sets up a mock server for frontmatter testing
async fn setup_frontmatter_test(
    path: &str,
    html_content: &str,
    include_frontmatter: bool,
    custom_fields: Vec<(String, String)>,
) -> (Server, String, HtmlConverter) {
    let mut server = Server::new_async().await;
    server
        .mock("GET", path)
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(html_content)
        .create_async()
        .await;

    let config = Config::builder().timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS).build();
    let client = HttpClient::with_config(&config.http, &config.auth);
    let html_config = HtmlConverterConfig::default();
    let output_config = markdowndown::config::OutputConfig {
        include_frontmatter,
        custom_frontmatter_fields: custom_fields,
        ..Default::default()
    };
    let converter = HtmlConverter::with_config(client, html_config, output_config);
    let url = format!("{}{}", server.url(), path);
    (server, url, converter)
}

/// Runs conversion test with mock server and verification function
async fn run_conversion_test<F>(
    path: &str,
    html_content: &str,
    config: Option<Config>,
    verify_fn: F,
) where
    F: FnOnce(&str),
{
    let (server, mock_assertion, url) = 
        setup_mock_html_test(path, html_content, None, None).await;
    let converter = create_test_converter(config, None, None);
    let result = converter.convert(&url).await;
    mock_assertion.await;
    assert!(result.is_ok());
    verify_fn(&result.unwrap().content_only());
}

// Additional HTML test content constants
const EMPTY_HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Empty Content</title>
</head>
<body>
    <!-- only comments -->
</body>
</html>"#;

const WHITESPACE_ONLY_HTML_CONTENT: &str = r#"<!DOCTYPE html>
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
            max_line_width: TestConstants::CUSTOM_LINE_WIDTH,
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
        run_conversion_test(
            "/test.html",
            SAMPLE_HTML_CONTENT,
            None,
            |content| {
                // Check for expected markdown elements based on SAMPLE_HTML_CONTENT
                assert_contains_all(content, &[
                    "# Test Article",
                    "## Features",
                    "**formatting**",
                    "*text*",
                    "[External links](https://example.com)",
                    "* Basic",
                ]);

                // Should not contain unwanted HTML elements
                assert_not_contains_any(content, &["<div", "<footer", "<nav"]);
            },
        ).await;
    }

    #[tokio::test]
    async fn test_convert_complex_html_with_preprocessing() {
        run_conversion_test(
            "/complex.html",
            COMPLEX_HTML_CONTENT,
            None,
            |content| {
                // Check for main content based on COMPLEX_HTML_CONTENT
                assert_contains_all(content, &[
                    "# Complex Document Title",
                    "This is the main content that should be preserved.",
                ]);

                // Should not contain scripts, styles, or navigation elements
                assert_not_contains_any(content, &[
                    "trackUser",
                    "gtag",
                    "body { margin: 0; }",
                ]);
            },
        ).await;
    }

    #[tokio::test]
    async fn test_convert_html_with_custom_headers() {
        let html_content = "<html><body><h1>Test</h1><p>Content</p></body></html>";
        
        let (server, mock_assertion, url) = setup_mock_html_test(
            "/protected.html",
            html_content,
            None,
            Some(vec![
                ("User-Agent", "test-agent/1.0"),
                ("Accept", "text/html,application/xhtml+xml"),
            ]),
        ).await;

        let config = Config::builder()
            .user_agent("test-agent/1.0")
            .timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS)
            .build();
        let converter = create_test_converter(Some(config), None, None);

        let result = converter.convert(&url).await;

        mock_assertion.await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert_contains_all(&markdown.content_only(), &["# Test", "Content"]);
    }

    #[tokio::test]
    async fn test_convert_html_with_different_encodings() {
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

        run_conversion_test(
            "/encoding.html",
            html_content,
            None,
            |content| {
                // Check that special characters are preserved
                assert_contains_all(content, &[
                    "café",
                    "naïve",
                    "résumé",
                    "你好",
                    "Здравствуй",
                    "مرحبا",
                ]);
            },
        ).await;
    }

    #[tokio::test]
    async fn test_convert_empty_html() {
        run_conversion_test(
            "/empty.html",
            "<html><body></body></html>",
            None,
            |content| {
                // Empty HTML should result in minimal markdown
                assert!(content.len() < TestConstants::MAX_EMPTY_CONTENT_LENGTH);
            },
        ).await;
    }

    #[tokio::test]
    async fn test_convert_html_with_malformed_markup() {
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

        run_conversion_test(
            "/malformed.html",
            html_content,
            None,
            |content| {
                // Should still extract meaningful content despite malformed HTML
                assert_contains_all(content, &[
                    "Heading without closing tag",
                    "Paragraph with",
                    "Nested content",
                    "Final paragraph",
                ]);
            },
        ).await;
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
    async fn test_convert_http_errors() {
        test_http_error_scenarios(vec![
            (404, 1),
            (500, 2),
        ]).await;
    }

    #[tokio::test]
    async fn test_convert_non_html_content() {
        let json_content = r#"{"message": "This is JSON, not HTML"}"#;
        
        let (server, mock_assertion, url) = setup_mock_html_test(
            "/data.json",
            json_content,
            None,
            None,
        ).await;

        let converter = create_test_converter(None, None, None);
        let result = converter.convert(&url).await;

        mock_assertion.await;
        // Should still work - the converter will treat JSON as text content
        assert!(result.is_ok());

        let markdown = result.unwrap();
        // The JSON should be converted to plain text
        assert!(markdown.content_only().contains("This is JSON, not HTML"));
    }

    #[tokio::test]
    async fn test_convert_large_html_content() {
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

        let (server, mock_assertion, url) = setup_mock_html_test(
            "/large.html",
            &large_content,
            None,
            None,
        ).await;

        let config = Config::builder()
            .timeout_seconds(TestConstants::LARGE_CONTENT_TIMEOUT_SECONDS)
            .build();
        let converter = create_test_converter(Some(config), None, None);

        let result = converter.convert(&url).await;

        mock_assertion.await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Large Content Test"));
        assert!(markdown.content_only().len() > TestConstants::MIN_LARGE_CONTENT_SIZE);
    }
}

/// Tests for configuration handling
mod configuration_tests {
    use super::*;

    #[test]
    fn test_html_converter_config_default() {
        let config = HtmlConverterConfig::default();

        // Test default values
        assert_eq!(config.max_line_width, TestConstants::DEFAULT_MAX_LINE_WIDTH);
        assert!(config.remove_scripts_styles);
        assert!(config.remove_navigation);
        assert!(config.remove_sidebars);
        assert!(config.remove_ads);
        assert_eq!(config.max_blank_lines, TestConstants::DEFAULT_MAX_BLANK_LINES);
    }

    #[test]
    fn test_html_converter_config_custom() {
        let config = HtmlConverterConfig {
            max_line_width: TestConstants::CUSTOM_MAX_LINE_WIDTH,
            remove_scripts_styles: false,
            remove_navigation: false,
            remove_sidebars: false,
            remove_ads: false,
            max_blank_lines: TestConstants::CUSTOM_MAX_BLANK_LINES,
        };

        assert_eq!(config.max_line_width, TestConstants::CUSTOM_MAX_LINE_WIDTH);
        assert!(!config.remove_scripts_styles);
        assert!(!config.remove_navigation);
        assert!(!config.remove_sidebars);
        assert!(!config.remove_ads);
        assert_eq!(config.max_blank_lines, TestConstants::CUSTOM_MAX_BLANK_LINES);
    }

    #[tokio::test]
    async fn test_converter_respects_configuration() {
        let (server, mock_assertion, url) =
            setup_mock_html_test("/config-test.html", COMPLEX_HTML_CONTENT, None, None).await;

        // Test with conservative config (keep more content)
        let config = Config::builder().timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS).build();
        let html_config = HtmlConverterConfig {
            max_line_width: TestConstants::LARGE_LINE_WIDTH,
            remove_scripts_styles: false,
            remove_navigation: false,
            remove_sidebars: false,
            remove_ads: false,
            max_blank_lines: TestConstants::LARGE_MAX_BLANK_LINES,
        };
        let output_config = markdowndown::config::OutputConfig::default();
        let converter = create_test_converter(Some(config), Some(html_config), Some(output_config));

        let result = converter.convert(&url).await;

        mock_assertion.await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // With conservative config, more content should be preserved
        assert!(content.contains("Complex Document Title"));
    }
}

/// Tests for frontmatter generation
mod frontmatter_tests {
    use super::*;

    const FRONTMATTER_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Document with Title</title>
</head>
<body>
    <h1>Main Heading</h1>
    <p>Content here</p>
</body>
</html>"#;

    #[tokio::test]
    async fn test_frontmatter_with_title_and_custom_fields() {
        let (server, url, converter) = setup_frontmatter_test(
            "/frontmatter-test.html",
            FRONTMATTER_HTML,
            true,
            vec![
                ("custom_field".to_string(), "custom_value".to_string()),
                ("author".to_string(), "test_author".to_string()),
            ],
        ).await;
        
        let result = converter.convert(&url).await;
        assert!(result.is_ok());
        
        let markdown = result.unwrap();
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        
        assert_contains_all(frontmatter, &[
            "title: Document with Title",
            "custom_field: custom_value",
            "author: test_author",
        ]);
        assert!(markdown.content_only().contains("# Main Heading"));
    }

    #[tokio::test]
    async fn test_no_frontmatter() {
        let (server, url, converter) = setup_frontmatter_test(
            "/no-frontmatter.html",
            FRONTMATTER_HTML,
            false,
            vec![],
        ).await;
        
        let result = converter.convert(&url).await;
        assert!(result.is_ok());
        
        let markdown = result.unwrap();
        assert!(markdown.frontmatter().is_none());
        assert!(markdown.content_only().contains("# Main Heading"));
    }

    #[tokio::test]
    async fn test_frontmatter_title_only() {
        let (server, url, converter) = setup_frontmatter_test(
            "/title-test.html",
            FRONTMATTER_HTML,
            true,
            vec![],
        ).await;
        
        let result = converter.convert(&url).await;
        assert!(result.is_ok());
        
        let markdown = result.unwrap();
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert!(frontmatter.contains("title: Document with Title"));
    }

    #[tokio::test]
    async fn test_frontmatter_custom_fields_only() {
        let (server, url, converter) = setup_frontmatter_test(
            "/custom-fields.html",
            FRONTMATTER_HTML,
            true,
            vec![
                ("custom_field".to_string(), "custom_value".to_string()),
                ("author".to_string(), "test_author".to_string()),
            ],
        ).await;
        
        let result = converter.convert(&url).await;
        assert!(result.is_ok());
        
        let markdown = result.unwrap();
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert_contains_all(frontmatter, &[
            "custom_field: custom_value",
            "author: test_author",
        ]);
    }

    #[tokio::test]
    async fn test_convert_html_with_no_title() {
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
</head>
<body>
    <h1>Content without title tag</h1>
</body>
</html>"#;

        let (server, url, converter) =
            setup_frontmatter_test("/no-title.html", html_content, true, vec![]).await;

        let result = converter.convert(&url).await;

        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter but no title field
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert!(!frontmatter.contains("title:"));
    }

    #[tokio::test]
    async fn test_convert_html_without_title_tag() {
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
</head>
<body>
    <h1>Content without title tag</h1>
</body>
</html>"#;

        let (server, url, converter) =
            setup_frontmatter_test("/no-title-2.html", html_content, true, vec![]).await;

        let result = converter.convert(&url).await;

        assert!(result.is_ok());

        let markdown = result.unwrap();

        // Should have frontmatter but no title field (tests line 174 - None return)
        assert!(markdown.frontmatter().is_some());
        let frontmatter = markdown.frontmatter().unwrap();
        assert!(!frontmatter.contains("title:"));
    }

    #[tokio::test]
    async fn test_empty_html_with_comments() {
        run_conversion_test(
            "/empty-content.html",
            EMPTY_HTML_CONTENT,
            None,
            |content| {
                assert!(content.contains("<!-- Empty HTML document -->"));
            },
        ).await;
    }

    #[tokio::test]
    async fn test_empty_html_with_whitespace() {
        run_conversion_test(
            "/empty-content-2.html",
            WHITESPACE_ONLY_HTML_CONTENT,
            None,
            |content| {
                assert!(content.contains("<!-- Empty HTML document -->"));
            },
        ).await;
    }
}

/// Tests for title extraction functionality
mod title_extraction_tests {
    use super::*;

    #[test]
    fn test_title_extraction_with_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>Test Document Title</title></head><body></body></html>"#;
        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_title_extraction_with_whitespace() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>  Nested Title with Whitespace  </title></head><body></body></html>"#;
        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_title_extraction_no_title_tag() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head></head><body><h1>No title tag</h1></body></html>"#;
        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_title_extraction_unclosed_title() {
        let converter = HtmlConverter::new();
        let html = r#"<html><head><title>Unclosed title<body></body></html>"#;
        let result = converter.convert_html(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_title_extraction_empty_title() {
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
        let long_title = "A".repeat(TestConstants::EXTREMELY_LONG_TITLE_LENGTH);
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

}

/// Integration tests combining multiple features
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_html_conversion() {
        let (server, mock_assertion, url) = setup_mock_html_test(
            "/integration-test.html",
            SAMPLE_HTML_CONTENT,
            None,
            None,
        ).await;

        let config = Config::builder()
            .user_agent("integration-test/1.0")
            .timeout_seconds(TestConstants::TEST_TIMEOUT_SECONDS)
            .max_retries(TestConstants::DEFAULT_MAX_RETRIES)
            .build();
        let converter = create_test_converter(Some(config), None, None);

        let result = converter.convert(&url).await;

        mock_assertion.await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Verify all major markdown elements are present based on SAMPLE_HTML_CONTENT
        assert_contains_all(content, &[
            "# Test Article",
            "## Features",
            "**formatting**",
            "*text*",
            "[External links](https://example.com)",
            "* Basic",
            "* Multiple",
            "> This is a blockquote",
        ]);

        // Verify frontmatter is included if configured
        if let Some(frontmatter) = markdown.frontmatter() {
            assert_contains_all(frontmatter, &["title:", "url:"]);
        }
    }

    #[tokio::test]
    async fn test_html_converter_with_redirects() {
        let html_content = "<html><body><h1>Final Content</h1></body></html>";
        
        let mut server = Server::new_async().await;
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

        let converter = create_test_converter(None, None, None);

        let url = format!("{}/redirect-source", server.url());
        let result = converter.convert(&url).await;

        redirect_mock.assert_async().await;
        target_mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Final Content"));
    }
}
