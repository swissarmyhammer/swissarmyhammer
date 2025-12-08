//! Common test utilities and shared functionality for markdowndown tests.

use chrono::{DateTime, Utc};
use markdowndown::types::{Frontmatter, Markdown, Url, UrlType};

/// Helper function to create a valid test URL
pub fn create_test_url(url_str: &str) -> Url {
    Url::new(url_str.to_string()).expect("Test URL should be valid")
}

/// Helper function to create valid test markdown
pub fn create_test_markdown(content: &str) -> Markdown {
    Markdown::new(content.to_string()).expect("Test markdown should be valid")
}

/// Helper function to create test frontmatter
pub fn create_test_frontmatter(url_str: &str, exporter: &str) -> Frontmatter {
    Frontmatter {
        source_url: create_test_url(url_str),
        exporter: exporter.to_string(),
        date_downloaded: Utc::now(),
    }
}

/// Common test URLs for different types
pub const TEST_HTML_URL: &str = "https://example.com/article.html";
pub const TEST_GOOGLE_DOCS_URL: &str = "https://docs.google.com/document/d/abc123/edit";
pub const TEST_OFFICE365_URL: &str = "https://company.sharepoint.com/sites/team/Document.docx";
pub const TEST_GITHUB_ISSUE_URL: &str = "https://github.com/owner/repo/issues/123";
pub const TEST_GITHUB_PR_URL: &str = "https://github.com/owner/repo/pull/456";

/// Sample markdown content for testing
pub const SAMPLE_MARKDOWN: &str = r#"# Test Document

This is a test document with various markdown elements.

## Features

- Lists
- **Bold text**
- *Italic text*
- [Links](https://example.com)

```rust
fn main() {
    println!("Hello, world!");
}
```

> This is a blockquote

| Column 1 | Column 2 |
|----------|----------|
| Cell 1   | Cell 2   |
"#;

/// Sample HTML content for conversion testing
pub const SAMPLE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Document</title>
</head>
<body>
    <h1>Test Document</h1>
    <p>This is a test document with various HTML elements.</p>
    <h2>Features</h2>
    <ul>
        <li>Lists</li>
        <li><strong>Bold text</strong></li>
        <li><em>Italic text</em></li>
        <li><a href="https://example.com">Links</a></li>
    </ul>
    <pre><code>fn main() {
    println!("Hello, world!");
}</code></pre>
    <blockquote>This is a blockquote</blockquote>
    <table>
        <tr><th>Column 1</th><th>Column 2</th></tr>
        <tr><td>Cell 1</td><td>Cell 2</td></tr>
    </table>
</body>
</html>"#;

/// Helper to assert that two datetime values are close (within 1 second)
pub fn assert_datetime_close(actual: DateTime<Utc>, expected: DateTime<Utc>) {
    let diff = (actual - expected).num_seconds().abs();
    assert!(diff <= 1, "Datetime difference too large: {diff} seconds");
}

/// Helper to validate that content is valid markdown
pub fn validate_markdown_content(content: &str) -> bool {
    !content.trim().is_empty() && Markdown::new(content.to_string()).is_ok()
}

/// Helper to validate that a URL is properly formatted
pub fn validate_url_format(url: &str) -> bool {
    Url::new(url.to_string()).is_ok()
}

/// Test URL patterns for different types
pub fn test_urls_by_type() -> Vec<(UrlType, Vec<&'static str>)> {
    vec![
        (
            UrlType::Html,
            vec![
                "https://example.com",
                "http://test.org/page.html",
                "https://blog.example.com/post/123",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_url() {
        let url = create_test_url(TEST_HTML_URL);
        assert_eq!(url.as_str(), TEST_HTML_URL);
    }

    #[test]
    fn test_create_test_markdown() {
        let markdown = create_test_markdown(SAMPLE_MARKDOWN);
        assert_eq!(markdown.as_str(), SAMPLE_MARKDOWN);
    }

    #[test]
    fn test_create_test_frontmatter() {
        let frontmatter = create_test_frontmatter(TEST_HTML_URL, "markdowndown");
        assert_eq!(frontmatter.source_url.as_str(), TEST_HTML_URL);
        assert_eq!(frontmatter.exporter, "markdowndown");
    }

    #[test]
    fn test_validate_markdown_content() {
        assert!(validate_markdown_content(SAMPLE_MARKDOWN));
        assert!(!validate_markdown_content(""));
        assert!(!validate_markdown_content("   \n\t  "));
    }

    #[test]
    fn test_validate_url_format() {
        assert!(validate_url_format(TEST_HTML_URL));
        assert!(validate_url_format(TEST_GOOGLE_DOCS_URL));
        assert!(!validate_url_format("not-a-url"));
        assert!(!validate_url_format("ftp://example.com"));
    }

    #[test]
    fn test_datetime_close_assertion() {
        let now = Utc::now();
        let almost_now = now + chrono::Duration::milliseconds(500);
        assert_datetime_close(now, almost_now);
    }
}
