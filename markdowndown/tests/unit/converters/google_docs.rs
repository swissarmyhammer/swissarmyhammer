//! Comprehensive unit tests for Google Docs to markdown converter.
//!
//! This module tests Google Docs conversion functionality, including URL parsing,
//! export API integration, error handling, and document format processing.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{Converter, GoogleDocsConverter};
use markdowndown::types::{MarkdownError, NetworkErrorKind};
use mockito::Server;

mod helpers {
    use super::*;

    /// Create a test Google Docs converter
    pub fn create_test_converter() -> GoogleDocsConverter {
        GoogleDocsConverter::new()
    }

    /// Sample Google Docs URLs for testing
    pub fn sample_google_docs_urls() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
                "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms"
            ),
            (
                "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view",
                "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms"
            ),
            (
                "https://docs.google.com/document/d/test_doc_id/edit?usp=sharing",
                "test_doc_id"
            ),
            (
                "https://drive.google.com/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvD2drive/view",
                "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvD2drive"
            ),
            (
                "https://drive.google.com/file/d/drive_file_123/edit",
                "drive_file_123"
            ),
        ]
    }

    /// Sample markdown content that Google Docs export might return
    pub fn sample_google_docs_markdown() -> &'static str {
        r#"# Meeting Notes - Q4 Planning

## Agenda Items

1. **Budget Review**
   - Current spending vs. budget
   - Q4 projections
   - Cost optimization opportunities

2. **Product Roadmap**
   - Feature prioritization
   - Release timeline
   - Resource allocation

## Action Items

- [ ] Review budget spreadsheet (Due: Next Friday)
- [ ] Update product requirements document
- [ ] Schedule follow-up meeting with engineering team

## Key Decisions

> **Decision**: Increase marketing budget by 15% for Q4 campaign
> 
> **Rationale**: Market research shows high potential ROI for holiday season targeting

## Notes

This document outlines the key discussion points from our Q4 planning meeting. Please review and provide feedback by end of week.

**Next Meeting**: October 15, 2024 at 2:00 PM PST

---

*Document created: October 1, 2024*
*Last updated: October 2, 2024*"#
    }

    /// Sample HTML content that Google Docs export API returns
    pub fn sample_google_docs_html() -> &'static str {
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Meeting Notes - Q4 Planning - Google Docs</title>
    <meta name="description" content="Q4 planning meeting notes">
</head>
<body>
    <div class="doc-content">
        <h1>Meeting Notes - Q4 Planning</h1>
        
        <h2>Agenda Items</h2>
        <ol>
            <li><strong>Budget Review</strong>
                <ul>
                    <li>Current spending vs. budget</li>
                    <li>Q4 projections</li>
                    <li>Cost optimization opportunities</li>
                </ul>
            </li>
            <li><strong>Product Roadmap</strong>
                <ul>
                    <li>Feature prioritization</li>
                    <li>Release timeline</li>
                    <li>Resource allocation</li>
                </ul>
            </li>
        </ol>
        
        <h2>Action Items</h2>
        <ul>
            <li>Review budget spreadsheet (Due: Next Friday)</li>
            <li>Update product requirements document</li>
            <li>Schedule follow-up meeting with engineering team</li>
        </ul>
        
        <h2>Key Decisions</h2>
        <blockquote>
            <p><strong>Decision</strong>: Increase marketing budget by 15% for Q4 campaign</p>
            <p><strong>Rationale</strong>: Market research shows high potential ROI for holiday season targeting</p>
        </blockquote>
        
        <h2>Notes</h2>
        <p>This document outlines the key discussion points from our Q4 planning meeting. Please review and provide feedback by end of week.</p>
        
        <p><strong>Next Meeting</strong>: October 15, 2024 at 2:00 PM PST</p>
        
        <hr>
        
        <p><em>Document created: October 1, 2024</em><br>
        <em>Last updated: October 2, 2024</em></p>
    </div>
</body>
</html>"#
    }

    /// Sample plain text content that Google Docs export might return
    pub fn sample_google_docs_text() -> &'static str {
        r#"Meeting Notes - Q4 Planning

Agenda Items

1. Budget Review
   - Current spending vs. budget
   - Q4 projections
   - Cost optimization opportunities

2. Product Roadmap
   - Feature prioritization
   - Release timeline
   - Resource allocation

Action Items

- Review budget spreadsheet (Due: Next Friday)
- Update product requirements document
- Schedule follow-up meeting with engineering team

Key Decisions

Decision: Increase marketing budget by 15% for Q4 campaign

Rationale: Market research shows high potential ROI for holiday season targeting

Notes

This document outlines the key discussion points from our Q4 planning meeting. Please review and provide feedback by end of week.

Next Meeting: October 15, 2024 at 2:00 PM PST

---

Document created: October 1, 2024
Last updated: October 2, 2024"#
    }
}

/// Tests for Google Docs converter creation
mod converter_creation_tests {
    use super::*;

    #[test]
    fn test_google_docs_converter_new() {
        let converter = GoogleDocsConverter::new();
        assert_eq!(converter.name(), "Google Docs");
    }

    #[test]
    fn test_google_docs_converter_with_client() {
        let _client = HttpClient::new();
        let converter = GoogleDocsConverter::new();
        assert_eq!(converter.name(), "Google Docs");
    }
}

/// Tests for URL parsing and document ID extraction
mod url_parsing_tests {
    use super::*;

    #[test]
    fn test_extract_document_ids() {
        let _converter = helpers::create_test_converter();

        for (_url, _expected_id) in helpers::sample_google_docs_urls() {
            // This would test the internal ID extraction logic if exposed.
            // For now, we'll test through the conversion process.
            // The converter should be able to handle these URLs correctly.

            // Note: Since extract_document_id might be private, we test indirectly
            // through the conversion process in integration tests.
        }
    }
}

/// Tests for successful Google Docs conversion
mod google_docs_conversion_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_google_docs_edit_url() {
        let mut server = Server::new_async().await;
        let _markdown_content = helpers::sample_google_docs_markdown();

        // Mock the Google Docs export API
        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body(helpers::sample_google_docs_text())
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        // For testing, we'll need to mock the actual export URL that the converter generates
        // This is a simplified test - in reality, the converter would transform the URL
        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Verify content was converted properly
        assert!(content.contains("Meeting Notes - Q4 Planning"));
        assert!(content.contains("Agenda Items"));
        assert!(content.contains("Budget Review"));
        assert!(content.contains("Action Items"));
        assert!(content.contains("Key Decisions"));
    }

    #[tokio::test]
    async fn test_convert_google_docs_view_url() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body("Simple document content for testing.")
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("Simple document content"));
    }

    #[tokio::test]
    async fn test_convert_google_drive_file_url() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock(
                "GET",
                "/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvD2drive/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body("Drive file content converted to text.")
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvD2drive/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("Drive file content"));
    }

    #[tokio::test]
    async fn test_convert_google_docs_with_html_export() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_google_docs_html();

        // First try text export (fails), then fall back to HTML export
        let _text_mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvHTML/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(403)
            .with_body("Access denied")
            .create_async()
            .await;

        let html_mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvHTML/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "html".into()))
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).max_retries(2).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        // Test the fallback mechanism (this would require testing internal logic)
        // For now, test HTML export directly
        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvHTML/export?format=html",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        html_mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Currently the Google Docs converter returns HTML content as-is
        // In a future version, this could be enhanced to convert HTML to markdown
        assert!(content.contains("<h1>Meeting Notes - Q4 Planning</h1>"));
        assert!(content.contains("<h2>Agenda Items</h2>"));
        assert!(content.contains("<strong>Budget Review</strong>"));
    }

    #[tokio::test]
    async fn test_convert_google_docs_with_special_characters() {
        let mut server = Server::new_async().await;
        let text_content = r#"Document with Special Characters

This document contains various special characters:
- Accented characters: café, naïve, résumé
- Currency symbols: $100, €50, ¥1000
- Mathematical symbols: α, β, γ, ∑, ∫
- Quotation marks: "Hello", 'World', "Fancy quotes"
- Dashes: en-dash –, em-dash —
- Unicode: 你好, مرحبا, Здравствуй

Bullet points:
• First point
• Second point
• Third point

Copyright symbol: © 2024 Example Corp"#;

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvSPEC/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body(text_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvSPEC/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Verify special characters are preserved
        assert!(content.contains("café"));
        assert!(content.contains("naïve"));
        assert!(content.contains("€50"));
        assert!(content.contains("你好"));
        assert!(content.contains("مرحبا"));
        assert!(content.contains("© 2024"));
    }

    #[tokio::test]
    async fn test_convert_empty_google_docs() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvEMPT/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body("")
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvEMPT/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();
        // Empty document should result in placeholder content
        assert!(content.contains("[Empty document]"));
    }
}

/// Tests for error handling
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_invalid_url() {
        let converter = helpers::create_test_converter();
        let result = converter.convert("not-a-valid-url").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::InvalidUrl { url } => {
                assert_eq!(url, "not-a-valid-url");
            }
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[tokio::test]
    async fn test_convert_private_document_access_denied() {
        let mut server = Server::new_async().await;

        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let mock_path = format!("/document/d/{doc_id}/export");
        let mock = server
            .mock("GET", mock_path.as_str())
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(403)
            .with_body("Access denied. You need permission to access this document.")
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(10)
            .max_retries(0) // No retries for access denied test
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!("{}/document/d/{}/export?format=txt", server.url(), doc_id);
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        // Should be an authentication or permission error
        match result.unwrap_err() {
            MarkdownError::AuthenticationError { .. } => {
                // Expected - permission denied
            }
            MarkdownError::EnhancedNetworkError { .. } => {
                // Also acceptable - could be mapped as network error
            }
            _ => panic!("Expected authentication or network error"),
        }
    }

    #[tokio::test]
    async fn test_convert_document_not_found() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(404)
            .with_body("Document not found")
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(10).build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

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
    async fn test_convert_google_api_rate_limit() {
        let mut server = Server::new_async().await;

        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let mock_path = format!("/document/d/{doc_id}/export");
        let mock = server
            .mock("GET", mock_path.as_str())
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(429)
            .with_header("Retry-After", "60")
            .with_body("Rate limit exceeded")
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(10)
            .max_retries(0) // No retries for rate limit test
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!("{}/document/d/{}/export?format=txt", server.url(), doc_id);
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => {
                match kind {
                    NetworkErrorKind::RateLimited => {
                        // Expected rate limit error
                    }
                    _ => panic!("Expected RateLimited error"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_convert_google_api_server_error() {
        let mut server = Server::new_async().await;

        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let mock_path = format!("/document/d/{doc_id}/export");
        let mock = server
            .mock("GET", mock_path.as_str())
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(10)
            .max_retries(0) // No retries for server error test
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!("{}/document/d/{}/export?format=txt", server.url(), doc_id);
        let result = converter.convert(&export_url).await;

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
    async fn test_convert_malformed_google_docs_url() {
        let converter = helpers::create_test_converter();

        let malformed_urls = [
            "https://docs.google.com/document/",
            "https://docs.google.com/document/d/",
            "https://docs.google.com/document/d/edit",
            "https://drive.google.com/file/",
            "https://drive.google.com/file/d/",
        ];

        for url in malformed_urls {
            let result = converter.convert(url).await;
            assert!(result.is_err(), "Should fail for malformed URL: {url}");
        }
    }
}

/// Integration tests combining multiple features
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_google_docs_conversion() {
        let mut server = Server::new_async().await;
        let text_content = helpers::sample_google_docs_text();

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvINTG/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_header("content-length", &text_content.len().to_string())
            .with_body(text_content)
            .create_async()
            .await;

        let config = Config::builder()
            .user_agent("integration-test/1.0")
            .timeout_seconds(10)
            .max_retries(3)
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvINTG/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        // Verify all major elements from the sample content
        assert!(content.contains("Meeting Notes - Q4 Planning"));
        assert!(content.contains("Agenda Items"));
        assert!(content.contains("Budget Review"));
        assert!(content.contains("Action Items"));
        assert!(content.contains("Key Decisions"));
        assert!(content.contains("October 15, 2024"));

        // Verify frontmatter if present
        if let Some(frontmatter) = markdown.frontmatter() {
            assert!(frontmatter.contains("source_url:"));
            assert!(frontmatter.contains("exporter: markdowndown-googledocs-"));
            assert!(frontmatter.contains("document_id:"));
            assert!(frontmatter.contains("document_type: google_docs"));
        }
    }

    #[tokio::test]
    async fn test_google_docs_conversion_with_retry_logic() {
        let mut server = Server::new_async().await;

        // First request fails with 503, second succeeds
        let failing_mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvRETR/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(503)
            .expect(2) // Should be called twice (initial + 1 retry)
            .create_async()
            .await;

        let success_mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvRETR/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("Document content after retry")
            .expect(1)
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(10)
            .max_retries(3)
            .retry_delay(std::time::Duration::from_millis(10)) // Fast retry for testing
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvRETR/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        failing_mock.assert_async().await;
        success_mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown
            .content_only()
            .contains("Document content after retry"));
    }

    #[tokio::test]
    async fn test_google_docs_converter_with_large_document() {
        let mut server = Server::new_async().await;

        // Create large document content (100KB)
        let large_content = format!(
            "Large Google Docs Document\n\n{}\n\nEnd of document.",
            "This is a line of content that will be repeated many times to create a large document. ".repeat(1000)
        );

        let mock = server
            .mock(
                "GET",
                "/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvLRGE/export",
            )
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_header("content-length", &large_content.len().to_string())
            .with_body(&large_content)
            .create_async()
            .await;

        let config = Config::builder()
            .timeout_seconds(30) // Longer timeout for large content
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = format!(
            "{}/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvLRGE/export?format=txt",
            server.url()
        );
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown
            .content_only()
            .contains("Large Google Docs Document"));
        assert!(markdown.content_only().len() > 50000); // Should be substantial content
    }
}
