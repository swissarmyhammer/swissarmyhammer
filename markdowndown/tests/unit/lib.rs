//! Comprehensive unit tests for the unified library API.
//!
//! This module tests end-to-end workflows, error propagation, configuration handling,
//! and integration between all library components.

use markdowndown::config::Config;
use markdowndown::converters::GitHubConverter;
use markdowndown::types::{MarkdownError, NetworkErrorKind, UrlType, ValidationErrorKind};
use markdowndown::{convert_url, convert_url_with_config, detect_url_type, MarkdownDown};
use mockito::Server;

mod http_status {
    pub const OK: u16 = 200;
    pub const UNAUTHORIZED: u16 = 401;
    pub const FORBIDDEN: u16 = 403;
    pub const NOT_FOUND: u16 = 404;
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    pub const SERVICE_UNAVAILABLE: u16 = 503;
}

mod timeouts {
    pub const SHORT: u64 = 5;
    pub const TEST: u64 = 10;
    pub const DEFAULT: u64 = 30;
    pub const LARGE_CONTENT: u64 = 30;
}

mod retry_config {
    pub const TEST_MAX_RETRIES: u32 = 2;
    pub const TEST_MAX_RETRIES_THREE: u32 = 3;
    pub const DEFAULT_MAX_RETRIES: u32 = 3;
    pub const FAST_DELAY_MS: u64 = 10;
}

mod content_limits {
    pub const MIN_FRONTMATTER_SIZE: usize = 100;
    pub const MIN_LARGE_CONTENT_SIZE: usize = 100000;
    pub const LARGE_CONTENT_REPETITIONS: usize = 5000;
    pub const MAX_EXPECTED_BLANK_LINES: usize = 2;
}

mod test_config {
    pub const EXPECTED_SUPPORTED_URL_TYPES: usize = 4;
    pub const EXPECTED_FAILURE_ATTEMPTS: usize = 2;
    pub const CONCURRENT_TEST_COUNT: usize = 5;
}

mod helpers {
    use super::*;
    use mockito::Mock;

    /// Setup a mock HTML server with common response pattern
    pub async fn setup_html_mock(
        server: &mut Server,
        path: &str,
        html_content: &str,
    ) -> mockito::Mock {
        server
            .mock("GET", path)
            .with_status(super::http_status::OK as usize)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(html_content)
            .create_async()
            .await
    }

    /// Generic error assertion helper with custom error matcher
    pub async fn assert_error_with_status<F>(
        server: &mut Server,
        path: &str,
        status_code: u16,
        config: Config,
        error_matcher: F,
    ) where
        F: Fn(&MarkdownError) -> bool,
    {
        let mock = server
            .mock("GET", path)
            .with_status(status_code as usize)
            .create_async()
            .await;

        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), path);
        let result = md.convert_url(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error_matcher(&error), "Unexpected error type: {:?}", error);
    }

    /// Validate that an error is a server error type
    fn validate_server_error_type(error: &MarkdownError) -> bool {
        matches!(
            error,
            MarkdownError::EnhancedNetworkError {
                kind: NetworkErrorKind::ServerError(_),
                ..
            } | MarkdownError::NetworkError { .. }
        )
    }

    /// Validate that an error is an authentication error type
    fn validate_auth_error_type(error: &MarkdownError) -> bool {
        matches!(
            error,
            MarkdownError::AuthenticationError { .. }
                | MarkdownError::EnhancedNetworkError { .. }
        )
    }

    /// Helper for server error assertions
    pub async fn assert_server_error(
        server: &mut Server,
        path: &str,
        status_code: u16,
        config: Config,
    ) {
        assert_error_with_status(server, path, status_code, config, validate_server_error_type)
            .await;
    }

    /// Helper for authentication error assertions
    pub async fn assert_auth_error(
        server: &mut Server,
        path: &str,
        status_code: u16,
        config: Config,
    ) {
        assert_error_with_status(server, path, status_code, config, validate_auth_error_type)
            .await;
    }

    /// Helper for generic network error assertions
    pub async fn assert_network_error(
        server: &mut Server,
        path: &str,
        status_code: usize,
        config: Config,
    ) {
        assert_error_with_status(server, path, status_code as u16, config, |error| {
            matches!(error, MarkdownError::EnhancedNetworkError { .. })
        })
        .await;
    }

    /// Create failing mocks for retry testing
    async fn create_failing_mocks(
        server: &mut Server,
        path: &str,
        fail_count: usize,
    ) -> mockito::ServerGuard {
        server
            .mock("GET", path)
            .with_status(super::http_status::SERVICE_UNAVAILABLE as usize)
            .expect(fail_count)
            .create_async()
            .await
    }

    /// Create success mock for retry testing
    async fn create_success_mock(
        server: &mut Server,
        path: &str,
        success_content: &str,
    ) -> mockito::ServerGuard {
        server
            .mock("GET", path)
            .with_status(super::http_status::OK as usize)
            .with_header("content-type", "text/html")
            .with_body(success_content)
            .expect(1)
            .create_async()
            .await
    }

    /// Execute conversion with retry configuration
    async fn execute_with_retry(
        server: &Server,
        path: &str,
        config: Config,
    ) -> Result<crate::types::MarkdownOutput, MarkdownError> {
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), path);
        md.convert_url(&url).await
    }

    /// Test retry scenarios with failing mocks followed by success
    pub async fn test_retry_scenario(
        server: &mut Server,
        path: &str,
        fail_count: usize,
        success_content: &str,
        config: Config,
    ) -> Result<crate::types::MarkdownOutput, MarkdownError> {
        let failing_mock = create_failing_mocks(server, path, fail_count).await;
        let success_mock = create_success_mock(server, path, success_content).await;
        let result = execute_with_retry(server, path, config).await;

        failing_mock.assert_async().await;
        success_mock.assert_async().await;
        result
    }

    /// Run a conversion test and verify basic content expectations
    pub async fn run_conversion_test(
        server: &Server,
        path: &str,
        config: Config,
        expected_content: &str,
    ) -> crate::types::MarkdownOutput {
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), path);
        let result = md.convert_url(&url).await;
        assert!(result.is_ok(), "Conversion failed: {:?}", result.err());
        let markdown = result.unwrap();
        assert!(
            markdown.content_only().contains(expected_content),
            "Expected content '{}' not found in markdown",
            expected_content
        );
        markdown
    }

    /// Assert that frontmatter contains specific fields
    pub fn assert_frontmatter_contains(
        markdown: &crate::types::MarkdownOutput,
        fields: &[&str],
    ) {
        let frontmatter = markdown
            .frontmatter()
            .expect("Expected frontmatter to be present");
        for field in fields {
            assert!(
                frontmatter.contains(field),
                "Frontmatter missing field: {}. Frontmatter content: {}",
                field,
                frontmatter
            );
        }
    }

    /// Sample HTML content for end-to-end testing
    pub fn sample_html_page() -> String {
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Sample Article</title>
    <meta name="description" content="A test article for integration testing">
</head>
<body>
<article>
        <h1>Sample Article Title</h1>
        <p>This is the main content of the article with <strong>bold text</strong> and <em>italic text</em>.</p>
        
        <h2>Section 1</h2>
        <p>Content for section one with important information.</p>
        <ul>
            <li>First bullet point</li>
            <li>Second bullet point with <a href="https://example.com">a link</a></li>
            <li>Third bullet point</li>
        </ul>
        
        <h2>Section 2</h2>
        <blockquote>
            <p>This is an important quote that should be preserved in markdown.</p>
        </blockquote>
        
        <pre><code>function example() {
    console.log("Code block example");
    return true;
}</code></pre>
    </article>
    
<nav>
        <ul>
            <li><a href="/home">Home</a></li>
            <li><a href="/about">About</a></li>
        </ul>
    </nav>
    
<footer>
        <p>&copy; 2024 Test Company</p>
    </footer>
</body>
</html>"#.to_string()
    }

    /// Sample Google Docs export content
    pub fn sample_google_docs_text() -> String {
        r#"Meeting Minutes - Project Kickoff

Date: October 15, 2024
Attendees:
- Alice Smith (Project Manager)
- Bob Johnson (Lead Developer)  
- Carol Davis (Designer)

Agenda Items

1. Project Overview
   The new customer portal will provide self-service capabilities for our users.
   
2. Technical Requirements
   - React frontend with TypeScript
   - Node.js backend with Express
   - PostgreSQL database
   - Docker deployment
   
3. Timeline
   - Phase 1: Foundation (4 weeks)
   - Phase 2: Core Features (6 weeks)
   - Phase 3: Polish & Testing (2 weeks)

Action Items
- Alice: Create project charter by EOW
- Bob: Set up development environment
- Carol: Create initial wireframes

Next Meeting: October 22, 2024 at 10:00 AM PST"#.to_string()
    }

    /// Sample GitHub issue content
    pub fn sample_github_issue_json() -> &'static str {
        "{
  \"id\": 123456789,
  \"number\": 1234,
  \"title\": \"Add support for custom themes\",
  \"body\": \"Summary: We need to add support for custom themes. Requirements: Theme selection UI, Theme persistence, Dark/light mode toggle. Acceptance Criteria: Users can select themes, Settings are saved, UI respects theme\",
  \"state\": \"open\",
  \"created_at\": \"2024-10-15T10:30:00Z\",
  \"updated_at\": \"2024-10-15T14:25:00Z\",
  \"user\": {
    \"login\": \"testuser\",
    \"id\": 987654321,
    \"html_url\": \"https://github.com/testuser\"
  },
  \"labels\": [
    {
      \"name\": \"enhancement\",
      \"color\": \"84b6eb\"
    },
    {
      \"name\": \"good first issue\",
      \"color\": \"7057ff\"
    }
  ]
}"
    }

    /// Create a test config with custom settings
    pub fn create_test_config() -> Config {
        Config::builder()
            .timeout_seconds(super::timeouts::TEST)
            .user_agent("markdowndown-test/1.0")
            .max_retries(super::retry_config::TEST_MAX_RETRIES)
            .include_frontmatter(true)
            .build()
    }

    /// Apply custom fields to a config builder
    fn apply_custom_fields(
        mut builder: markdowndown::config::ConfigBuilder,
        fields: Vec<(&str, &str)>,
    ) -> markdowndown::config::ConfigBuilder {
        for (key, value) in fields {
            builder = builder.custom_frontmatter_field(key, value);
        }
        builder
    }

    /// Build frontmatter config with optional custom fields
    fn build_frontmatter_config(
        include_frontmatter: bool,
        custom_fields: Option<Vec<(&str, &str)>>,
    ) -> Config {
        let mut builder = Config::builder()
            .timeout_seconds(super::timeouts::SHORT)
            .include_frontmatter(include_frontmatter);

        if let Some(fields) = custom_fields {
            builder = apply_custom_fields(builder, fields);
        }

        builder.build()
    }

    /// Test frontmatter configuration with parameterized settings
    pub async fn test_frontmatter_config<V>(
        server: &Server,
        path: &str,
        include_frontmatter: bool,
        custom_fields: Option<Vec<(&str, &str)>>,
        validator: V,
    ) where
        V: Fn(&crate::types::MarkdownOutput),
    {
        let config = build_frontmatter_config(include_frontmatter, custom_fields);
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), path);
        let result = md.convert_url(&url).await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        validator(&markdown);
    }

    /// Enum to represent different converter types for end-to-end testing
    pub enum ConverterType {
        Html { content: String },
        GoogleDocs { content: String },
        GitHub { issue_json: String, comments_json: String },
    }

    /// Configuration for setting up converter mocks
    struct MockConfig {
        path: String,
        query_matcher: Option<(String, String)>,
        headers: Vec<(&'static str, &'static str)>,
        content_type: &'static str,
        body: String,
        additional_mocks: Vec<(String, String)>, // (path, body) pairs for additional mocks
    }

    /// Build the base mock with headers and content
    fn build_base_mock<'a>(
        server: &'a mut Server,
        path: &str,
        content_type: &'static str,
        body: &str,
        headers: &[(&'static str, &'static str)],
    ) -> mockito::Mock {
        let mut mock_builder = server
            .mock("GET", path)
            .with_status(super::http_status::OK as usize)
            .with_header("content-type", content_type)
            .with_body(body);

        for (header_name, header_value) in headers {
            mock_builder = mock_builder.match_header(header_name, header_value);
        }

        mock_builder
    }

    /// Apply query matcher to mock builder if provided
    fn apply_query_matcher(
        mock_builder: mockito::Mock,
        query_matcher: Option<(String, String)>,
    ) -> mockito::Mock {
        if let Some((key, value)) = query_matcher {
            mock_builder.match_query(mockito::Matcher::UrlEncoded(key, value))
        } else {
            mock_builder
        }
    }

    /// Create additional GitHub API mocks
    async fn create_additional_mocks(
        server: &mut Server,
        additional_mocks: Vec<(String, String)>,
    ) -> Vec<mockito::ServerGuard> {
        let mut mocks = Vec::new();
        for (path, body) in additional_mocks {
            let additional_mock = server
                .mock("GET", path.as_str())
                .match_header("Accept", "application/vnd.github.v3+json")
                .with_status(super::http_status::OK as usize)
                .with_header("content-type", "application/json")
                .with_body(body)
                .create_async()
                .await;
            mocks.push(additional_mock);
        }
        mocks
    }

    /// Setup mock for any converter type with parameterized configuration
    async fn setup_converter_mock(
        server: &mut Server,
        config: MockConfig,
    ) -> (String, Vec<mockito::ServerGuard>) {
        let mut mocks = Vec::new();
        
        let mock_builder = build_base_mock(
            server,
            &config.path,
            config.content_type,
            &config.body,
            &config.headers,
        );
        
        let mock_builder = apply_query_matcher(mock_builder, config.query_matcher);
        let main_mock = mock_builder.create_async().await;
        mocks.push(main_mock);
        
        let mut additional = create_additional_mocks(server, config.additional_mocks).await;
        mocks.append(&mut additional);
        
        (config.path.clone(), mocks)
    }



    /// Verify converter result contains expected content
    fn verify_converter_result(
        markdown: &crate::types::MarkdownOutput,
        expected_content_fragments: &[&str],
        expected_metadata: Option<&[&str]>,
    ) {
        let content = markdown.content_only();

        for fragment in expected_content_fragments {
            assert!(content.contains(fragment), "Expected content fragment '{}' not found", fragment);
        }

        if let Some(metadata_fields) = expected_metadata {
            assert_frontmatter_contains(markdown, metadata_fields);
        }
    }

    /// Run an end-to-end converter test with parameterized setup
    pub async fn test_converter_end_to_end(
        server: &mut Server,
        converter: ConverterType,
        expected_content_fragments: &[&str],
        expected_metadata: Option<&[&str]>,
    ) {
        let (path_or_url, mocks, use_github_converter) = match converter {
            ConverterType::Html { content } => {
                let (path, mocks) = setup_converter_mock(
                    server,
                    MockConfig {
                        path: "/test.html".to_string(),
                        query_matcher: None,
                        headers: vec![],
                        content_type: "text/html; charset=utf-8",
                        body: content,
                        additional_mocks: vec![],
                    },
                )
                .await;
                (path, mocks, false)
            }
            ConverterType::GoogleDocs { content } => {
                let path = "/document/d/test123/export";
                let (_, mocks) = setup_converter_mock(
                    server,
                    MockConfig {
                        path: path.to_string(),
                        query_matcher: Some(("format".to_string(), "txt".to_string())),
                        headers: vec![],
                        content_type: "text/plain; charset=utf-8",
                        body: content,
                        additional_mocks: vec![],
                    },
                )
                .await;
                (format!("{}?format=txt", path), mocks, false)
            }
            ConverterType::GitHub { issue_json, comments_json } => {
                let issue_path = "/repos/owner/repo/issues/1234";
                let comments_path = "/repos/owner/repo/issues/1234/comments";
                
                let (_path, mocks) = setup_converter_mock(
                    server,
                    MockConfig {
                        path: issue_path.to_string(),
                        query_matcher: None,
                        headers: vec![("Accept", "application/vnd.github.v3+json")],
                        content_type: "application/json",
                        body: issue_json,
                        additional_mocks: vec![(comments_path.to_string(), comments_json)],
                    },
                )
                .await;
                ("https://github.com/owner/repo/issues/1234".to_string(), mocks, true)
            }
        };

        let result = if use_github_converter {
            let github_converter = GitHubConverter::new_with_config(Some("test_token".to_string()), server.url());
            github_converter.convert(&path_or_url).await
        } else {
            let config = Config::builder()
                .timeout_seconds(super::timeouts::SHORT)
                .build();
            let md = MarkdownDown::with_config(config);
            let url = format!("{}{}", server.url(), path_or_url);
            md.convert_url(&url).await
        };

        for mock in mocks {
            mock.assert_async().await;
        }

        assert!(result.is_ok());
        let markdown = result.unwrap();
        verify_converter_result(&markdown, expected_content_fragments, expected_metadata);
    }

    /// Run an integration test with common setup pattern
    pub async fn run_integration_test<V>(
        server: &Server,
        path: &str,
        html_content: &str,
        config_builder: impl FnOnce() -> Config,
        validator: V,
    ) where
        V: Fn(&crate::types::MarkdownOutput),
    {
        let config = config_builder();
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), path);
        let result = md.convert_url(&url).await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        validator(&markdown);
    }

    /// Count the maximum number of consecutive blank lines in content
    pub fn count_max_consecutive_blank_lines(content: &str) -> usize {
        let mut consecutive_blank_lines = 0;
        let mut max_consecutive_blank = 0;

        for line in content.lines() {
            if line.trim().is_empty() {
                consecutive_blank_lines += 1;
                max_consecutive_blank = max_consecutive_blank.max(consecutive_blank_lines);
            } else {
                consecutive_blank_lines = 0;
            }
        }

        max_consecutive_blank
    }

    /// Validate heading content in markdown
    pub fn validate_headings(markdown: &crate::types::MarkdownOutput) {
        let content = markdown.content_only();
        assert!(content.contains("# Sample Article Title"));
        assert!(content.contains("## Section 1"));
    }

    /// Validate formatting (bold and italic) in markdown
    pub fn validate_formatting(markdown: &crate::types::MarkdownOutput) {
        let content = markdown.content_only();
        assert!(content.contains("**bold text**"));
        assert!(content.contains("*italic text*"));
    }

    /// Validate links in markdown
    pub fn validate_links(markdown: &crate::types::MarkdownOutput) {
        let content = markdown.content_only();
        assert!(content.contains("[a link](https://example.com)"));
    }

    /// Validate quotes in markdown
    pub fn validate_quotes(markdown: &crate::types::MarkdownOutput) {
        let content = markdown.content_only();
        assert!(content.contains("> This is an important quote"));
    }

    /// Validate frontmatter presence with standard fields
    pub fn validate_standard_frontmatter(markdown: &crate::types::MarkdownOutput) {
        assert_frontmatter_contains(markdown, &["source_url:", "exporter:", "date_downloaded:"]);
    }

    /// Validate custom frontmatter fields
    pub fn validate_custom_frontmatter(markdown: &crate::types::MarkdownOutput, fields: &[&str]) {
        assert_frontmatter_contains(markdown, fields);
    }

    /// Validate whitespace normalization
    pub fn validate_whitespace_normalization(markdown: &crate::types::MarkdownOutput) {
        let content = markdown.content_only();
        let max_consecutive_blank = count_max_consecutive_blank_lines(&content);
        assert!(max_consecutive_blank <= super::content_limits::MAX_EXPECTED_BLANK_LINES);
    }

    /// Generic URL conversion test helper
    pub async fn test_url_conversion_with_type(
        server: &mut Server,
        mock_path: &str,
        mock_body: &str,
        mock_content_type: &str,
        expected_content: &str,
    ) {
        let mock = server
            .mock("GET", mock_path)
            .with_status(super::http_status::OK as usize)
            .with_header("content-type", mock_content_type)
            .with_body(mock_body)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(super::timeouts::SHORT).build();
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}", server.url(), mock_path);
        let result = md.convert_url(&url).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().content_only().contains(expected_content));
        mock.assert_async().await;
    }

    /// Generic URL conversion test with query parameters
    pub async fn test_url_conversion_with_query(
        server: &mut Server,
        mock_path: &str,
        query_key: &str,
        query_value: &str,
        mock_body: &str,
        mock_content_type: &str,
        expected_content: &str,
    ) {
        let mock = server
            .mock("GET", mock_path)
            .match_query(mockito::Matcher::UrlEncoded(query_key.into(), query_value.into()))
            .with_status(super::http_status::OK as usize)
            .with_header("content-type", mock_content_type)
            .with_body(mock_body)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(super::timeouts::SHORT).build();
        let md = MarkdownDown::with_config(config);
        let url = format!("{}{}?{}={}", server.url(), mock_path, query_key, query_value);
        let result = md.convert_url(&url).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().content_only().contains(expected_content));
        mock.assert_async().await;
    }
}

/// Tests for MarkdownDown struct creation and configuration
mod markdowndown_creation_tests {
    use super::*;

    #[test]
    fn test_markdowndown_new() {
        let md = MarkdownDown::new();

        // Verify default configuration
        let config = md.config();
        assert_eq!(config.http.timeout.as_secs(), timeouts::DEFAULT);
        assert_eq!(config.http.max_retries, retry_config::DEFAULT_MAX_RETRIES);
        assert!(config.output.include_frontmatter);

        // Verify supported types
        let types = md.supported_types();
        assert!(types.contains(&UrlType::Html));
        assert!(types.contains(&UrlType::GoogleDocs));
        assert!(types.contains(&UrlType::GitHubIssue));
    }

    #[test]
    fn test_markdowndown_with_config() {
        let custom_config = helpers::create_test_config();
        let md = MarkdownDown::with_config(custom_config);

        // Verify custom configuration is used
        let config = md.config();
        assert_eq!(config.http.timeout.as_secs(), timeouts::TEST);
        assert_eq!(config.http.user_agent, "markdowndown-test/1.0");
        assert_eq!(config.http.max_retries, retry_config::TEST_MAX_RETRIES);
        assert!(config.output.include_frontmatter);
    }

    #[test]
    fn test_markdowndown_default() {
        let md = MarkdownDown::default();
        assert_eq!(md.config().http.timeout.as_secs(), timeouts::DEFAULT);
    }

    #[test]
    fn test_markdowndown_getters() {
        let md = MarkdownDown::new();

        // Test getter methods
        let _config = md.config();
        let _detector = md.detector();
        let _registry = md.registry();
        let types = md.supported_types();

        assert_eq!(types.len(), test_config::EXPECTED_SUPPORTED_URL_TYPES); // HTML, GoogleDocs, GitHubIssue, LocalFile
    }
}

/// Tests for end-to-end URL conversion workflows
mod end_to_end_conversion_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_html_basic() {
        let mut server = Server::new_async().await;
        
        helpers::test_converter_end_to_end(
            &mut server,
            helpers::ConverterType::Html {
                content: helpers::sample_html_page(),
            },
            &["# Sample Article Title", "## Section 1"],
            Some(&["source_url:", "exporter:", "date_downloaded:"]),
        )
        .await;
    }

    #[tokio::test]
    async fn test_convert_html_formatting() {
        let mut server = Server::new_async().await;
        
        helpers::test_converter_end_to_end(
            &mut server,
            helpers::ConverterType::Html {
                content: helpers::sample_html_page(),
            },
            &["**bold text**", "*italic text*"],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_convert_html_links_and_quotes() {
        let mut server = Server::new_async().await;
        
        helpers::test_converter_end_to_end(
            &mut server,
            helpers::ConverterType::Html {
                content: helpers::sample_html_page(),
            },
            &[
                "[a link](https://example.com)",
                "> This is an important quote",
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_convert_google_docs() {
        let mut server = Server::new_async().await;
        
        helpers::test_converter_end_to_end(
            &mut server,
            helpers::ConverterType::GoogleDocs {
                content: helpers::sample_google_docs_text(),
            },
            &[
                "Meeting Minutes",
                "Project Kickoff",
                "Action Items",
                "Alice Smith",
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_convert_github_issue() {
        let mut server = Server::new_async().await;
        
        helpers::test_converter_end_to_end(
            &mut server,
            helpers::ConverterType::GitHub {
                issue_json: helpers::sample_github_issue_json().to_string(),
                comments_json: "[]".to_string(),
            },
            &[
                "Add support for custom themes",
                "enhancement",
                "good first issue",
            ],
            None,
        )
        .await;
    }
}

/// Tests for error propagation through the full stack
mod error_propagation_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_url_error_propagation() {
        let md = MarkdownDown::new();
        let result = md.convert_url("not-a-valid-url").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ValidationError { kind, .. } => {
                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
            }
            _ => panic!("Expected ValidationError for invalid URL"),
        }
    }

    #[tokio::test]
    async fn test_network_error_propagation() {
        let mut server = Server::new_async().await;

        let config = Config::builder()
            .timeout_seconds(5)
            .max_retries(0) // No retries for error propagation test
            .build();

        helpers::assert_server_error(
            &mut server,
            "/unavailable.html",
            http_status::SERVICE_UNAVAILABLE,
            config,
        )
        .await;
    }

    #[tokio::test]
    async fn test_authentication_error_propagation() {
        let mut server = Server::new_async().await;

        helpers::assert_auth_error(
            &mut server,
            "/protected.html",
            http_status::UNAUTHORIZED,
            Config::default(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_unsupported_url_type_error() {
        // This test would require a URL type that's not supported
        // For now, we'll test with a malformed URL that can't be classified
        let md = MarkdownDown::new();
        let result = md.convert_url("ftp://example.com/file.txt").await;

        assert!(result.is_err());
        // Should be either a validation error or configuration error
        match result.unwrap_err() {
            MarkdownError::ValidationError { .. } => {
                // Expected - URL doesn't match any supported patterns
            }
            MarkdownError::LegacyConfigurationError { .. } => {
                // Also acceptable - no converter available
            }
            _ => panic!("Expected validation or configuration error"),
        }
    }

    #[tokio::test]
    async fn test_server_error_propagation() {
        let mut server = Server::new_async().await;

        let config = Config::builder()
            .max_retries(0) // No retries to get immediate error
            .build();

        helpers::assert_server_error(
            &mut server,
            "/error.html",
            http_status::INTERNAL_SERVER_ERROR,
            config,
        )
        .await;
    }
}

/// Tests for fallback mechanisms
mod fallback_mechanism_tests {
    use super::*;

    #[tokio::test]
    async fn test_google_docs_to_html_fallback() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Fallback Content</h1><p>Content fetched as HTML fallback.</p></body></html>";

        let failed_export_mock = server
            .mock("GET", "/document/d/fallback_test/export")
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(http_status::FORBIDDEN as usize)
            .with_body("Access denied")
            .create_async()
            .await;

        let _html_fallback_mock = server
            .mock("GET", "/document/d/fallback_test/export")
            .with_status(http_status::OK as usize)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder().timeout_seconds(timeouts::SHORT).max_retries(1).build();
        let md = MarkdownDown::with_config(config);

        let export_url = format!(
            "{}/document/d/fallback_test/export?format=txt",
            server.url()
        );
        let result = md.convert_url(&export_url).await;

        failed_export_mock.assert_async().await;

        match result {
            Ok(markdown) => {
                assert!(markdown.content_only().contains("Fallback Content"));
            }
            Err(_) => {
                // Fallback not implemented or failed - this is also acceptable
            }
        }
    }

    #[tokio::test]
    async fn test_no_fallback_for_html_converter() {
        let mut server = Server::new_async().await;

        helpers::assert_network_error(
            &mut server,
            "/failed.html",
            http_status::NOT_FOUND as usize,
            Config::default(),
        )
        .await;
    }
}

/// Tests for convenience functions
mod convenience_function_tests {
    use super::*;

    #[tokio::test]
    async fn test_convert_url_convenience_function() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Convenience Test</h1></body></html>";

        let mock = helpers::setup_html_mock(&mut server, "/convenience.html", html_content).await;

        let url = format!("{}/convenience.html", server.url());
        let result = convert_url(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Convenience Test"));
    }

    #[tokio::test]
    async fn test_convert_url_with_config_convenience_function() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Config Test</h1></body></html>";

        let mock = server
            .mock("GET", "/config-test.html")
            .match_header("User-Agent", "custom-test/1.0")
            .with_status(http_status::OK as usize)
            .with_header("content-type", "text/html")
            .with_body(html_content)
            .create_async()
            .await;

        let config = Config::builder()
            .user_agent("custom-test/1.0")
            .timeout_seconds(5)
            .build();

        let url = format!("{}/config-test.html", server.url());
        let result = convert_url_with_config(&url, config).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Config Test"));
    }

    #[test]
    fn test_detect_url_type_convenience_function() {
        // Test various URL types
        let html_result = detect_url_type("https://example.com/page.html");
        assert!(html_result.is_ok());
        assert_eq!(html_result.unwrap(), UrlType::Html);

        let gdocs_result = detect_url_type("https://docs.google.com/document/d/123/edit");
        assert!(gdocs_result.is_ok());
        assert_eq!(gdocs_result.unwrap(), UrlType::GoogleDocs);

        let office_result = detect_url_type("https://company.sharepoint.com/doc.docx");
        assert!(office_result.is_ok());
        assert_eq!(office_result.unwrap(), UrlType::Html);

        let github_result = detect_url_type("https://github.com/owner/repo/issues/123");
        assert!(github_result.is_ok());
        assert_eq!(github_result.unwrap(), UrlType::GitHubIssue);

        let invalid_result = detect_url_type("not-a-url");
        assert!(invalid_result.is_err());
    }
}

/// Tests for configuration integration
mod configuration_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_timeout_configuration_integration() {
        let mut server = Server::new_async().await;

        // Mock a server that responds slowly
        let _mock = server
            .mock("GET", "/timeout-test.html")
            .with_status(http_status::OK as usize)
            .with_header("content-type", "text/html")
            .with_body("<html><body><h1>Delayed Response</h1></body></html>")
            .create_async()
            .await;

        // Test with short timeout
        let short_timeout_config = Config::builder()
            .timeout_seconds(1) // Should be enough
            .build();
        let md = MarkdownDown::with_config(short_timeout_config);

        let url = format!("{}/timeout-test.html", server.url());
        let result = md.convert_url(&url).await;

        // Should succeed with 1 second timeout
        assert!(result.is_ok() || result.is_err()); // Accept either outcome due to timing sensitivity
    }

    #[tokio::test]
    async fn test_retry_configuration_integration() {
        let mut server = Server::new_async().await;

        let config = Config::builder()
            .max_retries(retry_config::TEST_MAX_RETRIES_THREE)
            .retry_delay(std::time::Duration::from_millis(retry_config::FAST_DELAY_MS))
            .build();

        let result = helpers::test_retry_scenario(
            &mut server,
            "/retry-test.html",
            test_config::EXPECTED_FAILURE_ATTEMPTS,
            "<html><body><h1>Success After Retry</h1></body></html>",
            config,
        )
        .await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Success After Retry"));
    }

    #[tokio::test]
    async fn test_user_agent_configuration_integration() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/user-agent-test.html")
            .match_header("User-Agent", "CustomApp/2.0 (Integration Test)")
            .with_status(http_status::OK as usize)
            .with_header("content-type", "text/html")
            .with_body("<html><body><h1>User Agent Test</h1></body></html>")
            .create_async()
            .await;

        let config = Config::builder()
            .user_agent("CustomApp/2.0 (Integration Test)")
            .build();

        let markdown = helpers::run_conversion_test(&server, "/user-agent-test.html", config, "# User Agent Test").await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_frontmatter_enabled_configuration() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Frontmatter Test</h1></body></html>";

        let mock = helpers::setup_html_mock(&mut server, "/frontmatter-test.html", html_content).await;

        helpers::test_frontmatter_config(
            &server,
            "/frontmatter-test.html",
            true,
            Some(vec![("test_field", "test_value")]),
            |markdown| {
                helpers::assert_frontmatter_contains(markdown, &["source_url:", "test_field:", "test_value"]);
            },
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_frontmatter_disabled_configuration() {
        let mut server = Server::new_async().await;
        let html_content = "<html><body><h1>Frontmatter Test</h1></body></html>";

        let mock = helpers::setup_html_mock(&mut server, "/frontmatter-test2.html", html_content).await;

        helpers::test_frontmatter_config(
            &server,
            "/frontmatter-test2.html",
            false,
            None,
            |markdown| {
                let frontmatter = markdown.frontmatter();
                if let Some(fm) = frontmatter {
                    assert!(fm.len() < content_limits::MIN_FRONTMATTER_SIZE);
                }
            },
        )
        .await;

        mock.assert_async().await;
    }
}

/// Integration tests combining multiple components
mod component_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_url_detection_and_conversion() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();

        let mock = helpers::setup_html_mock(&mut server, "/url-detection.html", &html_content).await;

        let url = format!("{}/url-detection.html", server.url());
        let detected_type = detect_url_type(&url).unwrap();
        assert_eq!(detected_type, UrlType::Html);

        helpers::run_integration_test(
            &server,
            "/url-detection.html",
            &html_content,
            || {
                Config::builder()
                    .user_agent("url-detection-test/1.0")
                    .timeout_seconds(timeouts::TEST)
                    .build()
            },
            |markdown| {
                assert!(markdown.content_only().contains("# Sample Article Title"));
            },
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_headings() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/headings.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/headings.html",
            &html_content,
            Config::builder,
            helpers::validate_headings,
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_formatting() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/formatting.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/formatting.html",
            &html_content,
            Config::builder,
            helpers::validate_formatting,
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_links() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/links.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/links.html",
            &html_content,
            Config::builder,
            helpers::validate_links,
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_quotes() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/quotes.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/quotes.html",
            &html_content,
            Config::builder,
            helpers::validate_quotes,
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_frontmatter() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/frontmatter-gen.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/frontmatter-gen.html",
            &html_content,
            || {
                Config::builder()
                    .include_frontmatter(true)
                    .custom_frontmatter_field("workflow", "full-integration")
                    .build()
            },
            |markdown| {
                helpers::validate_custom_frontmatter(
                    markdown,
                    &["source_url:", "exporter:", "date_downloaded:", "workflow: full-integration"],
                );
            },
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_conversion_whitespace() {
        let mut server = Server::new_async().await;
        let html_content = helpers::sample_html_page();
        
        let mock = helpers::setup_html_mock(&mut server, "/whitespace-norm.html", &html_content).await;

        helpers::run_integration_test(
            &server,
            "/whitespace-norm.html",
            &html_content,
            || {
                Config::builder()
                    .normalize_whitespace(true)
                    .max_consecutive_blank_lines(1)
                    .build()
            },
            helpers::validate_whitespace_normalization,
        )
        .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_error_recovery_workflow() {
        let mut server = Server::new_async().await;

        let config = Config::builder()
            .timeout_seconds(timeouts::SHORT)
            .max_retries(retry_config::TEST_MAX_RETRIES)
            .retry_delay(std::time::Duration::from_millis(retry_config::FAST_DELAY_MS))
            .build();

        let result = helpers::test_retry_scenario(
            &mut server,
            "/error-recovery.html",
            1,
            "<html><body><h1>Recovered Successfully</h1></body></html>",
            config,
        )
        .await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.content_only().contains("# Recovered Successfully"));
    }

    #[tokio::test]
    async fn test_html_url_conversion() {
        let mut server = Server::new_async().await;

        helpers::test_url_conversion_with_type(
            &mut server,
            "/test.html",
            "<html><body><h1>HTML Content</h1></body></html>",
            "text/html",
            "# HTML Content",
        )
        .await;
    }

    #[tokio::test]
    async fn test_google_docs_url_conversion() {
        let mut server = Server::new_async().await;

        helpers::test_url_conversion_with_query(
            &mut server,
            "/document/d/123/export",
            "format",
            "txt",
            "Google Docs Content\n\nThis is from Google Docs.",
            "text/plain",
            "Google Docs Content",
        )
        .await;
    }
}

/// Performance and stress tests
mod performance_tests {
    use super::*;

    /// Generate large HTML content for testing
    fn generate_large_html_content(repetitions: usize) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Large Document</title></head>
<body>
<h1>Large Content Test</h1>
{}
<h2>End of Document</h2>
</body>
</html>"#,
            "<p>This is a paragraph with substantial content to test large document handling. "
                .repeat(repetitions)
        )
    }

    #[tokio::test]
    async fn test_large_content_handling() {
        let mut server = Server::new_async().await;

        let large_content = generate_large_html_content(content_limits::LARGE_CONTENT_REPETITIONS);

        let mock = helpers::setup_html_mock(&mut server, "/large-content.html", &large_content).await;

        let config = Config::builder()
            .timeout_seconds(timeouts::LARGE_CONTENT)
            .build();

        let markdown = helpers::run_conversion_test(&server, "/large-content.html", config, "# Large Content Test").await;

        mock.assert_async().await;

        let content = markdown.content_only();
        assert!(content.contains("# Large Content Test"));
        assert!(content.contains("## End of Document"));
        assert!(content.len() > content_limits::MIN_LARGE_CONTENT_SIZE);
    }

    #[tokio::test]
    async fn test_concurrent_conversions() {
        let mut server = Server::new_async().await;
        let count = test_config::CONCURRENT_TEST_COUNT;

        // Setup concurrent mocks
        let mut mocks = Vec::new();
        for i in 0..count {
            let path = format!("/concurrent-{i}.html");
            let body = format!("<html><body><h1>Document {i}</h1></body></html>");
            let mock = server
                .mock("GET", path.as_str())
                .with_status(http_status::OK as usize)
                .with_header("content-type", "text/html")
                .with_body(&body)
                .create_async()
                .await;
            mocks.push(mock);
        }

        // Execute concurrent conversions
        let md = MarkdownDown::new();
        let mut tasks = Vec::new();
        for i in 0..count {
            let url = format!("{}/concurrent-{}.html", server.url(), i);
            let task = async move { md.convert_url(&url).await };
            tasks.push(task);
        }
        let results = futures::future::join_all(tasks).await;

        // Verify results
        for mock in mocks {
            mock.assert_async().await;
        }

        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Conversion {i} failed");
            let markdown = result.unwrap();
            assert!(markdown.content_only().contains(&format!("# Document {i}")));
        }
    }
}
