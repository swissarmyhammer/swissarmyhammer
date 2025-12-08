//! Shared helper functions for converter testing.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{
    ConverterRegistry, GoogleDocsConverter, HtmlConverter, HtmlConverterConfig,
};
use markdowndown::types::UrlType;
use std::time::Duration;

/// Create a test HTTP client with fast settings for testing
pub fn create_test_http_client() -> HttpClient {
    let config = Config::builder()
        .retry_delay(Duration::from_millis(10))
        .timeout(Duration::from_secs(2))
        .build();
    HttpClient::with_config(&config.http, &config.auth)
}

/// Create a basic HTML converter for testing
pub fn create_html_converter() -> HtmlConverter {
    HtmlConverter::new()
}

/// Create an HTML converter with test client
pub fn create_html_converter_with_client(client: HttpClient) -> HtmlConverter {
    let config = HtmlConverterConfig::default();
    let output_config = markdowndown::config::OutputConfig::default();
    HtmlConverter::with_config(client, config, output_config)
}

/// Create a basic Google Docs converter for testing
pub fn create_google_docs_converter() -> GoogleDocsConverter {
    GoogleDocsConverter::new()
}

/// Create a basic converter registry for testing
pub fn create_converter_registry() -> ConverterRegistry {
    ConverterRegistry::new()
}

/// Create a configured converter registry for testing
pub fn create_configured_converter_registry() -> ConverterRegistry {
    let config = Config::builder().timeout_seconds(10).max_retries(2).build();
    let http_client = HttpClient::with_config(&config.http, &config.auth);
    let html_config = HtmlConverterConfig::default();
    let output_config = markdowndown::config::OutputConfig::default();
    ConverterRegistry::with_config(http_client, html_config, &output_config)
}

/// Get sample URLs for each converter type
pub fn sample_urls_by_converter() -> Vec<(UrlType, Vec<&'static str>)> {
    vec![
        (
            UrlType::Html,
            vec![
                "https://example.com/page.html",
                "https://blog.example.com/article",
                "https://news.site.com/story/123",
            ],
        ),
        (
            UrlType::GoogleDocs,
            vec![
                "https://docs.google.com/document/d/abc123/edit",
                "https://docs.google.com/document/d/xyz789",
                "https://docs.google.com/document/d/test123/edit#heading=h.abc",
            ],
        ),
        (
            UrlType::GitHubIssue,
            vec![
                "https://github.com/owner/repo/issues/123",
                "https://github.com/microsoft/vscode/pull/456",
                "https://github.com/rust-lang/rust/issues/789",
            ],
        ),
    ]
}

/// Sample HTML content for HTML converter testing
pub const SAMPLE_HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Article</title>
    <meta name="description" content="A test article for HTML conversion">
</head>
<body>
    <article>
        <h1>Test Article</h1>
        <p>This is a sample HTML document for testing the HTML converter.</p>
        
        <h2>Features</h2>
        <ul>
            <li>Basic <strong>formatting</strong></li>
            <li>Multiple <em>text</em> styles</li>
            <li><a href="https://example.com">External links</a></li>
        </ul>
        
        <blockquote>
            <p>This is a blockquote for testing.</p>
        </blockquote>
        
        <pre><code>function test() {
    return "Hello, world!";
}</code></pre>
    </article>
</body>
</html>"#;

/// Sample GitHub issue HTML content
pub const SAMPLE_GITHUB_ISSUE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Issue · owner/repo</title>
</head>
<body>
    <div class="issue">
        <h1>Test Issue Title</h1>
        <div class="issue-body">
            <p>This is a sample GitHub issue for testing.</p>
            <h2>Steps to Reproduce</h2>
            <ol>
                <li>Step one</li>
                <li>Step two</li>
                <li>Step three</li>
            </ol>
        </div>
    </div>
</body>
</html>"#;

/// Sample Google Docs HTML content
pub const SAMPLE_GOOGLE_DOCS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Document - Google Docs</title>
</head>
<body>
    <div class="doc-content">  
        <h1>Meeting Notes - Q4 Planning</h1>
        <p>These are sample meeting notes from Google Docs.</p>
        
        <h2>Agenda Items</h2>
        <ul>
            <li>Budget review</li>
            <li>Project timeline</li>
            <li>Resource allocation</li>
        </ul>
        
        <p><strong>Action Items:</strong></p>
        <ol>
            <li>Finalize budget by EOW</li>
            <li>Schedule team meetings</li>
        </ol>
    </div>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_converters() {
        let _html = create_html_converter();
        let _google = create_google_docs_converter();
        let _registry = create_converter_registry();
    }

    #[test]
    fn test_create_with_config() {
        let client = create_test_http_client();

        let _html = create_html_converter_with_client(client.clone());
        let _registry = create_configured_converter_registry();
    }

    #[test]
    fn test_sample_content_constants() {
        assert!(SAMPLE_HTML_CONTENT.len() > 100);
        assert!(SAMPLE_GITHUB_ISSUE_HTML.len() > 100);
        assert!(SAMPLE_GOOGLE_DOCS_HTML.len() > 100);
    }

    #[test]
    fn test_sample_urls() {
        let urls = sample_urls_by_converter();
        assert_eq!(urls.len(), 3);

        for (url_type, url_list) in urls {
            assert!(
                !url_list.is_empty(),
                "URL list should not be empty for {url_type:?}"
            );
        }
    }
}
