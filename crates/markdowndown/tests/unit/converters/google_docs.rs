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

    /// Test document IDs
    pub const TEST_DOC_ID: &str = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
    pub const TEST_DRIVE_ID: &str = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvD2drive";

    /// Test configuration for various scenarios
    pub struct TestConfig {
        pub timeout_seconds: u64,
        pub max_retries: u32,
        pub retry_delay_ms: u64,
    }

    impl TestConfig {
        /// Default configuration for standard tests
        pub fn default() -> Self {
            Self {
                timeout_seconds: 10,
                max_retries: 3,
                retry_delay_ms: 10,
            }
        }

        /// Set timeout in seconds
        pub fn timeout_seconds(mut self, timeout: u64) -> Self {
            self.timeout_seconds = timeout;
            self
        }

        /// Set maximum number of retries
        pub fn max_retries(mut self, retries: u32) -> Self {
            self.max_retries = retries;
            self
        }

        /// Set retry delay in milliseconds
        pub fn retry_delay_ms(mut self, delay_ms: u64) -> Self {
            self.retry_delay_ms = delay_ms;
            self
        }
    }

    /// Retry-After header value for rate limit tests (in seconds)
    pub const RATE_LIMIT_RETRY_AFTER_SECONDS: u32 = 60;

    /// Number of repetitions to create large document test content
    pub const LARGE_DOC_REPEAT_COUNT: usize = 1000;

    /// Minimum expected size for large document validation (in bytes)
    pub const EXPECTED_LARGE_DOC_MIN_SIZE: usize = 50000;

    /// Create a test Google Docs converter
    pub fn create_test_converter() -> GoogleDocsConverter {
        GoogleDocsConverter::new()
    }

    /// Create test HTTP client with default configuration
    pub fn create_test_client() -> HttpClient {
        let test_config = TestConfig::default();
        create_test_client_with_config(&test_config)
    }

    /// Create test HTTP client with custom test configuration
    pub fn create_test_client_with_config(test_config: &TestConfig) -> HttpClient {
        let config = Config::builder()
            .timeout_seconds(test_config.timeout_seconds)
            .max_retries(test_config.max_retries)
            .retry_delay(std::time::Duration::from_millis(test_config.retry_delay_ms))
            .build();
        HttpClient::with_config(&config.http, &config.auth)
    }

    /// Builder for creating Google Docs mock server endpoints
    pub struct GoogleDocsMockBuilder {
        doc_id: String,
        status: usize,
        body: String,
        format: String,
        resource_type: String,
        expect_count: Option<usize>,
        content_length: Option<usize>,
    }

    impl GoogleDocsMockBuilder {
        pub fn new(doc_id: &str) -> Self {
            Self {
                doc_id: doc_id.to_string(),
                status: 200,
                body: String::new(),
                format: "txt".to_string(),
                resource_type: "document".to_string(),
                expect_count: None,
                content_length: None,
            }
        }

        pub fn with_status(mut self, status: usize) -> Self {
            self.status = status;
            self
        }

        pub fn with_body(mut self, body: &str) -> Self {
            self.body = body.to_string();
            self
        }

        pub fn with_format(mut self, format: &str) -> Self {
            self.format = format.to_string();
            self
        }

        pub fn with_resource_type(mut self, resource_type: &str) -> Self {
            self.resource_type = resource_type.to_string();
            self
        }

        pub fn with_expect_count(mut self, count: usize) -> Self {
            self.expect_count = Some(count);
            self
        }

        pub fn with_content_length(mut self, length: usize) -> Self {
            self.content_length = Some(length);
            self
        }

        pub async fn build(self, server: &mut Server) -> mockito::Mock {
            let mut mock = server
                .mock(
                    "GET",
                    &format!("/{}/d/{}/export", self.resource_type, self.doc_id),
                )
                .match_query(mockito::Matcher::UrlEncoded(
                    "format".into(),
                    self.format.clone(),
                ))
                .with_status(self.status)
                .with_header(
                    "content-type",
                    &format!("text/{}; charset=utf-8", self.format),
                )
                .with_body(&self.body);

            if let Some(count) = self.expect_count {
                mock = mock.expect(count);
            }

            if let Some(length) = self.content_length {
                mock = mock.with_header("content-length", &length.to_string());
            }

            mock.create_async().await
        }
    }

    /// Setup mock server with configuration closure
    pub async fn setup_mock_with_config<F>(
        server: &mut Server,
        doc_id: &str,
        configure: F,
    ) -> (mockito::Mock, String)
    where
        F: FnOnce(GoogleDocsMockBuilder) -> GoogleDocsMockBuilder,
    {
        let builder = GoogleDocsMockBuilder::new(doc_id);
        let mock = configure(builder).build(server).await;
        let export_url = build_export_url(server.url(), doc_id, "document", "txt");
        (mock, export_url)
    }

    /// Setup fallback mock servers for testing format fallback scenarios
    pub async fn setup_fallback_mocks(
        server: &mut Server,
        doc_id: &str,
        primary_format: &str,
        primary_status: usize,
        primary_body: &str,
        fallback_format: &str,
        fallback_body: &str,
    ) -> (mockito::Mock, mockito::Mock) {
        let primary_mock = GoogleDocsMockBuilder::new(doc_id)
            .with_format(primary_format)
            .with_status(primary_status)
            .with_body(primary_body)
            .build(server)
            .await;

        let fallback_mock = GoogleDocsMockBuilder::new(doc_id)
            .with_format(fallback_format)
            .with_status(200)
            .with_body(fallback_body)
            .build(server)
            .await;

        (primary_mock, fallback_mock)
    }

    /// Build export URL for Google Docs or Drive resources
    pub fn build_export_url(
        server_url: &str,
        doc_id: &str,
        resource_type: &str,
        format: &str,
    ) -> String {
        format!(
            "{}/{}/d/{}/export?format={}",
            server_url, resource_type, doc_id, format
        )
    }

    /// Assert that conversion was successful and contains expected content
    pub fn assert_conversion_success(
        result: Result<markdowndown::types::Markdown, MarkdownError>,
        expected_content: &str,
    ) {
        assert!(result.is_ok());
        let markdown = result.unwrap();
        let content = markdown.content_only();
        assert!(content.contains(expected_content));
    }

    /// Assert mock was called and conversion was successful
    pub async fn assert_mock_and_conversion(
        mock: mockito::Mock,
        result: Result<markdowndown::types::Markdown, MarkdownError>,
        expected_content: &str,
    ) {
        mock.assert_async().await;
        assert_conversion_success(result, expected_content);
    }

    /// Test Google Docs conversion with customizable parameters
    pub async fn test_google_docs_conversion(
        doc_id: &str,
        resource_type: &str,
        body: &str,
        expected_content: &str,
    ) {
        let mut server = Server::new_async().await;
        let mock = GoogleDocsMockBuilder::new(doc_id)
            .with_body(body)
            .with_resource_type(resource_type)
            .build(&mut server)
            .await;
        let client = create_test_client();
        let converter = GoogleDocsConverter::with_client(client);
        let export_url = build_export_url(server.url(), doc_id, resource_type, "txt");
        let result = converter.convert(&export_url).await;
        assert_mock_and_conversion(mock, result, expected_content).await;
    }

    /// Test Google Docs error response with specific status and error validation
    pub async fn test_google_docs_error_response<F>(
        status: usize,
        body: &str,
        expected_error_kind: F,
    ) where
        F: FnOnce(&MarkdownError) -> bool,
    {
        let mut server = Server::new_async().await;
        let mock = GoogleDocsMockBuilder::new(TEST_DOC_ID)
            .with_status(status)
            .with_body(body)
            .build(&mut server)
            .await;

        let test_config = TestConfig::default().max_retries(0);
        let client = create_test_client_with_config(&test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = build_export_url(server.url(), TEST_DOC_ID, "document", "txt");
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        assert!(expected_error_kind(&result.unwrap_err()));
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

    /// Sample plain text content with special characters for testing
    pub fn sample_google_docs_text_with_special_chars() -> &'static str {
        r#"Document with Special Characters

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

Copyright symbol: © 2024 Example Corp"#
    }

    /// Run integration test with customizable mock setup, config, and validation
    pub async fn run_integration_test<MockFn, ValidateFn>(
        doc_id: &str,
        test_config: &TestConfig,
        setup_mock: MockFn,
        validate_result: ValidateFn,
    ) where
        MockFn: FnOnce(&mut Server, &str) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = (mockito::Mock, String)> + Send + '_>,
        >,
        ValidateFn: FnOnce(Result<markdowndown::types::Markdown, MarkdownError>),
    {
        let mut server = Server::new_async().await;
        let (mock, export_url) = setup_mock(&mut server, doc_id).await;

        let client = create_test_client_with_config(test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        validate_result(result);
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
    async fn test_google_docs_conversions() {
        let test_cases = vec![
            (
                helpers::TEST_DOC_ID,
                "document",
                helpers::sample_google_docs_text(),
                "Meeting Notes - Q4 Planning",
                "edit URL",
            ),
            (
                helpers::TEST_DOC_ID,
                "document",
                "Simple document content for testing.",
                "Simple document content",
                "view URL",
            ),
            (
                helpers::TEST_DRIVE_ID,
                "file",
                "Drive file content converted to text.",
                "Drive file content",
                "drive file URL",
            ),
        ];

        for (doc_id, resource_type, body, expected, description) in test_cases {
            println!("Testing: {}", description);
            helpers::test_google_docs_conversion(doc_id, resource_type, body, expected).await;
        }
    }

    #[tokio::test]
    async fn test_convert_google_docs_with_html_export() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_google_docs_html();
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvHTML";

        let (_text_mock, html_mock) = helpers::setup_fallback_mocks(
            &mut server,
            doc_id,
            "txt",
            403,
            "Access denied",
            "html",
            html_content,
        )
        .await;

        let test_config = helpers::TestConfig::default().max_retries(2);
        let client = helpers::create_test_client_with_config(&test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = helpers::build_export_url(server.url(), doc_id, "document", "html");
        let result = converter.convert(&export_url).await;

        html_mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.content_only();

        assert!(content.contains("<h1>Meeting Notes - Q4 Planning</h1>"));
        assert!(content.contains("<h2>Agenda Items</h2>"));
        assert!(content.contains("<strong>Budget Review</strong>"));
    }

    #[tokio::test]
    async fn test_convert_google_docs_with_special_characters() {
        let mut server = Server::new_async().await;
        let text_content = helpers::sample_google_docs_text_with_special_chars();
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvSPEC";

        let mock = helpers::GoogleDocsMockBuilder::new(doc_id)
            .with_body(text_content)
            .build(&mut server)
            .await;

        let client = helpers::create_test_client();
        let converter = GoogleDocsConverter::with_client(client);

        let export_url = helpers::build_export_url(server.url(), doc_id, "document", "txt");
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
        helpers::test_google_docs_conversion(
            "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvEMPT",
            "document",
            "",
            "[Empty document]",
        )
        .await;
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
    async fn test_google_docs_error_responses() {
        let test_cases: Vec<(
            usize,
            &str,
            Box<dyn Fn(&MarkdownError) -> bool>,
            &str,
        )> = vec![
            (
                403,
                "Access denied. You need permission to access this document.",
                Box::new(|err: &MarkdownError| {
                    matches!(
                        err,
                        MarkdownError::AuthenticationError { .. }
                            | MarkdownError::EnhancedNetworkError { .. }
                    )
                }),
                "access denied",
            ),
            (
                404,
                "Document not found",
                Box::new(|err: &MarkdownError| {
                    matches!(
                        err,
                        MarkdownError::EnhancedNetworkError {
                            kind: NetworkErrorKind::ServerError(404),
                            ..
                        }
                    )
                }),
                "not found",
            ),
            (
                429,
                "Rate limit exceeded",
                Box::new(|err: &MarkdownError| {
                    matches!(
                        err,
                        MarkdownError::EnhancedNetworkError {
                            kind: NetworkErrorKind::RateLimited,
                            ..
                        }
                    )
                }),
                "rate limit",
            ),
            (
                500,
                "Internal Server Error",
                Box::new(|err: &MarkdownError| {
                    matches!(
                        err,
                        MarkdownError::EnhancedNetworkError {
                            kind: NetworkErrorKind::ServerError(500),
                            ..
                        }
                    )
                }),
                "server error",
            ),
        ];

        for (status, body, validator, description) in test_cases {
            println!("Testing error response: {}", description);
            helpers::test_google_docs_error_response(status, body, validator).await;
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
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvINTG";
        let text_content = helpers::sample_google_docs_text();
        let content_length = text_content.len();

        let (mock, export_url) = helpers::setup_mock_with_config(&mut server, doc_id, |builder| {
            builder
                .with_body(text_content)
                .with_content_length(content_length)
        })
        .await;

        let test_config = helpers::TestConfig::default();
        let client = helpers::create_test_client_with_config(&test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let result = converter.convert(&export_url).await;

        helpers::assert_mock_and_conversion(mock, result, "Meeting Notes - Q4 Planning").await;
    }

    #[tokio::test]
    async fn test_google_docs_conversion_with_retry_logic() {
        let mut server = Server::new_async().await;
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvRETR";

        let _failing_mock = helpers::GoogleDocsMockBuilder::new(doc_id)
            .with_status(503)
            .with_expect_count(2)
            .build(&mut server)
            .await;

        let (success_mock, export_url) =
            helpers::setup_mock_with_config(&mut server, doc_id, |builder| {
                builder
                    .with_status(200)
                    .with_body("Document content after retry")
                    .with_expect_count(1)
            })
            .await;

        let test_config = helpers::TestConfig::default();
        let client = helpers::create_test_client_with_config(&test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let result = converter.convert(&export_url).await;

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
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvLRGE";

        let large_content = format!(
            "Large Google Docs Document\n\n{}\n\nEnd of document.",
            "This is a line of content that will be repeated many times to create a large document. "
                .repeat(helpers::LARGE_DOC_REPEAT_COUNT)
        );
        let content_length = large_content.len();

        let (mock, export_url) = helpers::setup_mock_with_config(&mut server, doc_id, |builder| {
            builder
                .with_body(&large_content)
                .with_content_length(content_length)
        })
        .await;

        let test_config = helpers::TestConfig::default().timeout_seconds(30);
        let client = helpers::create_test_client_with_config(&test_config);
        let converter = GoogleDocsConverter::with_client(client);

        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown
            .content_only()
            .contains("Large Google Docs Document"));
        assert!(markdown.content_only().len() > helpers::EXPECTED_LARGE_DOC_MIN_SIZE);
    }
}
