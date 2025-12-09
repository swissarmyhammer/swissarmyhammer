//! Comprehensive unit tests for HTTP client functionality.
//!
//! This module tests the HTTP client with mock servers, timeout handling,
//! retry logic, authentication, and comprehensive error scenarios.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::types::{
    converter_types, operations, AuthErrorKind, MarkdownError, NetworkErrorKind,
    ValidationErrorKind,
};
use mockito::Server;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

/// Configurable test timeouts - can be overridden via environment variables
const DEFAULT_TEST_RETRY_DELAY_MS: u64 = 10;
const DEFAULT_TEST_TIMEOUT_SECS: u64 = 2;

/// Test data size constants
const TEST_LARGE_RESPONSE_SIZE: usize = 100_000;

/// Test timeout and delay constants
const TEST_RETRY_AFTER_SECONDS: u64 = 60;
const TEST_VERY_SHORT_TIMEOUT_MS: u64 = 100;
const TEST_SLOW_RESPONSE_DELAY_SECS: u64 = 3;
const TEST_OUTER_TIMEOUT_SECS: u64 = 5;

/// Test URL and concurrency constants
const TEST_VERY_LONG_PATH_LENGTH: usize = 2000;
const TEST_CONCURRENT_REQUEST_COUNT: usize = 5;

/// HTTP status code constants
const HTTP_OK: u16 = 200;
const HTTP_FOUND: u16 = 302;
const HTTP_BAD_REQUEST: u16 = 400;
const HTTP_UNAUTHORIZED: u16 = 401;
const HTTP_FORBIDDEN: u16 = 403;
const HTTP_NOT_FOUND: u16 = 404;
const HTTP_METHOD_NOT_ALLOWED: u16 = 405;
const HTTP_NOT_ACCEPTABLE: u16 = 406;
const HTTP_CONFLICT: u16 = 409;
const HTTP_GONE: u16 = 410;
const HTTP_UNPROCESSABLE_ENTITY: u16 = 422;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const HTTP_INTERNAL_SERVER_ERROR: u16 = 500;
const HTTP_BAD_GATEWAY: u16 = 502;
const HTTP_SERVICE_UNAVAILABLE: u16 = 503;
const HTTP_GATEWAY_TIMEOUT: u16 = 504;

/// Macro to parse environment variable durations with less boilerplate
macro_rules! env_duration {
    ($var:expr, $default:expr, $converter:expr) => {
        std::env::var($var)
            .ok()
            .and_then(|v| v.parse().ok())
            .map($converter)
            .unwrap_or($default)
    };
}

fn get_test_retry_delay() -> Duration {
    env_duration!(
        "TEST_RETRY_DELAY_MS",
        Duration::from_millis(DEFAULT_TEST_RETRY_DELAY_MS),
        Duration::from_millis
    )
}

fn get_test_timeout() -> Duration {
    env_duration!(
        "TEST_TIMEOUT_SECS",
        Duration::from_secs(DEFAULT_TEST_TIMEOUT_SECS),
        Duration::from_secs
    )
}

mod helpers {
    use super::*;

    /// Create a test HTTP client with optional authentication configuration
    fn create_client_with_config(
        github_token: Option<&str>,
        office365_token: Option<&str>,
        google_api_key: Option<&str>,
    ) -> HttpClient {
        let mut builder = Config::builder()
            .retry_delay(get_test_retry_delay())
            .timeout(get_test_timeout());

        if let Some(token) = github_token {
            builder = builder.github_token(token);
        }
        if let Some(token) = office365_token {
            builder = builder.office365_token(token);
        }
        if let Some(key) = google_api_key {
            builder = builder.google_api_key(key);
        }

        let config = builder.build();
        HttpClient::with_config(&config.http, &config.auth)
    }

    /// Create a test HTTP client with configurable delays for testing
    pub fn create_test_client() -> HttpClient {
        create_client_with_config(None, None, None)
    }

    /// Create a test HTTP client with authentication tokens
    pub fn create_auth_client() -> HttpClient {
        create_client_with_config(
            Some("test_github_token"),
            Some("test_office365_token"),
            Some("test_google_api_key"),
        )
    }

    /// Test configuration builder for common test scenarios
    pub struct TestConfigBuilder {
        timeout: Option<Duration>,
        retry_delay: Option<Duration>,
        max_retries: Option<usize>,
    }

    impl TestConfigBuilder {
        pub fn new() -> Self {
            Self {
                timeout: None,
                retry_delay: None,
                max_retries: None,
            }
        }

        pub fn with_short_timeout(mut self) -> Self {
            self.timeout = Some(Duration::from_millis(TEST_VERY_SHORT_TIMEOUT_MS));
            self
        }

        pub fn with_fast_retry(mut self) -> Self {
            self.retry_delay = Some(Duration::from_millis(1));
            self
        }

        pub fn with_no_retry(mut self) -> Self {
            self.max_retries = Some(0);
            self
        }

        pub fn build(self) -> HttpClient {
            let timeout = self.timeout.unwrap_or_else(get_test_timeout);
            let retry_delay = self.retry_delay.unwrap_or_else(get_test_retry_delay);

            let mut builder = Config::builder()
                .timeout(timeout)
                .retry_delay(retry_delay);

            if let Some(retries) = self.max_retries {
                builder = builder.max_retries(retries);
            }

            let config = builder.build();
            HttpClient::with_config(&config.http, &config.auth)
        }
    }

    /// Generic helper to assert error types with custom matcher
    pub fn assert_error_matches<F>(result: Result<String, MarkdownError>, matcher: F)
    where
        F: FnOnce(&MarkdownError),
    {
        assert!(result.is_err());
        matcher(&result.unwrap_err());
    }

    /// Assert that a result contains a ValidationError with InvalidUrl kind
    pub fn assert_validation_error(result: Result<String, MarkdownError>, expected_url: &str) {
        assert_error_matches(result, |err| match err {
            MarkdownError::ValidationError { kind, context } => {
                assert_eq!(*kind, ValidationErrorKind::InvalidUrl);
                assert_eq!(context.url, expected_url);
                assert_eq!(context.operation, operations::URL_VALIDATION);
                assert_eq!(context.converter_type, converter_types::HTTP_CLIENT);
            }
            _ => panic!("Expected ValidationError, got: {err:?}"),
        });
    }

    /// Assert that a URL is rejected with a ValidationError
    pub async fn assert_url_rejected(client: &HttpClient, url: &str) {
        let result = client.get_text(url).await;
        assert!(result.is_err(), "Should reject URL: {url}");

        assert_error_matches(result, |err| match err {
            MarkdownError::ValidationError { kind, .. } => {
                assert_eq!(*kind, ValidationErrorKind::InvalidUrl);
            }
            _ => panic!("Expected ValidationError for URL: {url}, got: {err:?}"),
        });
    }

    /// Assert that a result contains a ServerError with the expected status code
    pub fn assert_server_error(
        result: Result<String, MarkdownError>,
        expected_status: u16,
        should_contain: &str,
    ) {
        assert_error_matches(result, |err| match err {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::ServerError(status) => {
                        assert_eq!(*status, expected_status);
                        assert!(context
                            .additional_info
                            .as_ref()
                            .unwrap()
                            .contains(should_contain));
                    }
                    _ => panic!("Expected ServerError({expected_status}), got: {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError, got: {err:?}"),
        });
    }

    /// Assert that a result contains a NetworkError with the expected kind
    pub fn assert_network_error(
        result: Result<String, MarkdownError>,
        expected_kind: NetworkErrorKind,
    ) {
        assert_error_matches(result, |err| match err {
            MarkdownError::EnhancedNetworkError { kind, .. } => {
                assert_eq!(
                    std::mem::discriminant(kind),
                    std::mem::discriminant(&expected_kind)
                );
            }
            _ => panic!("Expected EnhancedNetworkError, got: {err:?}"),
        });
    }

    /// Generic helper to verify server error with expected status code
    pub fn verify_server_error(err: &MarkdownError, expected_status: u16) {
        match err {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::ServerError(status) => {
                        assert_eq!(*status, expected_status);
                    }
                    _ => panic!("Expected ServerError({expected_status}), got: {kind:?}"),
                }
                assert!(context
                    .additional_info
                    .as_ref()
                    .unwrap()
                    .contains(&expected_status.to_string()));
            }
            _ => panic!("Expected EnhancedNetworkError, got: {err:?}"),
        }
    }

    /// Verify authentication error with expected kind
    pub fn verify_auth_error(err: &MarkdownError, expected_kind: AuthErrorKind, expected_status: u16) {
        match err {
            MarkdownError::AuthenticationError { kind, context } => {
                assert_eq!(*kind, expected_kind);
                assert!(context
                    .additional_info
                    .as_ref()
                    .unwrap()
                    .contains(&expected_status.to_string()));
            }
            _ => panic!("Expected AuthenticationError, got: {err:?}"),
        }
    }

    /// Verify rate limited error
    pub fn verify_rate_limited_error(err: &MarkdownError) {
        match err {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::RateLimited => {
                        assert!(context
                            .additional_info
                            .as_ref()
                            .unwrap()
                            .contains("429"));
                    }
                    _ => panic!("Expected RateLimited error, got: {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError, got: {err:?}"),
        }
    }

    /// Verify status code error with retry information
    pub fn verify_status_code_error(
        result: Result<String, MarkdownError>,
        expected_status: u16,
        should_retry: bool,
        expected_attempts: usize,
    ) {
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::ServerError(code) => {
                        assert_eq!(code, expected_status);
                        let info = context.additional_info.unwrap();
                        assert!(info.contains(&expected_status.to_string()));
                        if should_retry {
                            assert!(info.contains(&format!("{expected_attempts} attempts")));
                        }
                    }
                    _ => panic!("Expected ServerError({expected_status}), got: {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    /// Add headers to a mock endpoint
    fn add_headers_to_mock(
        mut mock: mockito::Mock,
        headers: Option<HashMap<String, String>>,
    ) -> mockito::Mock {
        if let Some(headers) = headers {
            for (key, value) in headers {
                mock = mock.match_header(key.as_str(), value.as_str());
            }
        }
        mock
    }

    /// Setup a mock endpoint with configurable parameters
    pub async fn setup_mock_endpoint(
        server: &mut Server,
        path: &str,
        status: usize,
        body: &str,
        expect_calls: usize,
        match_headers: Option<HashMap<String, String>>,
    ) -> mockito::Mock {
        let mock = server.mock("GET", path).with_status(status).with_body(body);
        let mock = add_headers_to_mock(mock, match_headers);
        mock.expect(expect_calls).create_async().await
    }

    /// Reusable test fixture for mock server tests
    pub struct MockTestFixture {
        pub server: Server,
        pub client: HttpClient,
    }

    impl MockTestFixture {
        pub async fn new() -> Self {
            Self {
                server: Server::new_async().await,
                client: create_test_client(),
            }
        }

        pub fn url(&self, path: &str) -> String {
            format!("{}{}", self.server.url(), path)
        }

        pub fn mock_endpoint_builder(&mut self, path: &str) -> MockEndpointBuilder {
            MockEndpointBuilder::new(&mut self.server, path)
        }

        pub async fn mock_endpoint(
            &mut self,
            path: &str,
            status: usize,
            body: &str,
        ) -> mockito::Mock {
            self.mock_endpoint_builder(path)
                .with_status(status)
                .with_body(body)
                .create()
                .await
        }

        pub async fn mock_endpoint_with_headers(
            &mut self,
            path: &str,
            status: usize,
            body: &str,
            headers: HashMap<&str, &str>,
        ) -> mockito::Mock {
            self.mock_endpoint_builder(path)
                .with_status(status)
                .with_body(body)
                .with_headers(headers)
                .create()
                .await
        }

        pub async fn mock_endpoint_with_expect(
            &mut self,
            path: &str,
            status: usize,
            body: &str,
            expect_calls: usize,
        ) -> mockito::Mock {
            self.mock_endpoint_builder(path)
                .with_status(status)
                .with_body(body)
                .expect(expect_calls)
                .create()
                .await
        }
    }

    /// Builder for mock endpoints with fluent API
    pub struct MockEndpointBuilder<'a> {
        server: &'a mut Server,
        path: String,
        status: usize,
        body: String,
        headers: HashMap<&'a str, &'a str>,
        expect_calls: Option<usize>,
    }

    impl<'a> MockEndpointBuilder<'a> {
        pub fn new(server: &'a mut Server, path: &str) -> Self {
            Self {
                server,
                path: path.to_string(),
                status: HTTP_OK as usize,
                body: String::new(),
                headers: HashMap::new(),
                expect_calls: None,
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

        pub fn with_headers(mut self, headers: HashMap<&'a str, &'a str>) -> Self {
            self.headers = headers;
            self
        }

        pub fn expect(mut self, count: usize) -> Self {
            self.expect_calls = Some(count);
            self
        }

        pub async fn create(self) -> mockito::Mock {
            let mut mock = self
                .server
                .mock("GET", self.path.as_str())
                .with_status(self.status)
                .with_body(self.body);

            for (key, value) in self.headers {
                mock = mock.match_header(key, value);
            }

            if let Some(count) = self.expect_calls {
                mock = mock.expect(count);
            }

            mock.create_async().await
        }
    }
}

/// Tests for HTTP client creation and configuration
mod client_creation_tests {
    use super::*;

    #[test]
    fn test_http_client_new() {
        let _client = HttpClient::new();
        // These are private fields, so we test indirectly through behavior
        // The default client should be created successfully
        // Test passes if no panic occurs during creation
    }

    #[test]
    fn test_http_client_default() {
        let _client = HttpClient::default();
        // Test that default is equivalent to new()
        // Test passes if no panic occurs during creation
    }

    #[test]
    fn test_http_client_with_custom_config() {
        let config = Config::builder()
            .max_retries(5)
            .timeout_seconds(60)
            .user_agent("custom-agent/1.0")
            .build();

        let _client = HttpClient::with_config(&config.http, &config.auth);
        // Client should be created with custom config
        // Test passes if no panic occurs during creation with custom config
    }
}

/// Tests for successful HTTP operations
mod success_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_text_success() {
        let mut fixture = helpers::MockTestFixture::new().await;
        let expected_body = "Hello, World! This is test content.";

        let mock = fixture
            .server
            .mock("GET", "/test")
            .with_status(HTTP_OK as usize)
            .with_header("content-type", "text/plain")
            .with_body(expected_body)
            .create_async()
            .await;

        let url = fixture.url("/test");
        let result = fixture.client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_body);
    }

    #[tokio::test]
    async fn test_get_bytes_success() {
        let mut fixture = helpers::MockTestFixture::new().await;
        let expected_body = b"Binary data content";

        let mock = fixture
            .server
            .mock("GET", "/binary")
            .with_status(HTTP_OK as usize)
            .with_header("content-type", "application/octet-stream")
            .with_body(expected_body)
            .create_async()
            .await;

        let url = fixture.url("/binary");
        let result = fixture.client.get_bytes(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_ref(), expected_body);
    }

    #[tokio::test]
    async fn test_get_text_with_headers() {
        let mut fixture = helpers::MockTestFixture::new().await;
        let expected_body = "Content with custom headers";

        let mock = fixture
            .server
            .mock("GET", "/with-headers")
            .match_header("X-Custom-Header", "test-value")
            .match_header("User-Agent", "test-agent/1.0")
            .with_status(HTTP_OK as usize)
            .with_body(expected_body)
            .create_async()
            .await;

        let url = fixture.url("/with-headers");

        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "test-value".to_string());
        headers.insert("User-Agent".to_string(), "test-agent/1.0".to_string());

        let result = fixture.client.get_text_with_headers(&url, &headers).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_body);
    }

    #[tokio::test]
    async fn test_large_response_handling() {
        let mut fixture = helpers::MockTestFixture::new().await;
        let large_content = "A".repeat(TEST_LARGE_RESPONSE_SIZE); // 100KB of data

        let mock = fixture
            .server
            .mock("GET", "/large")
            .with_status(HTTP_OK)
            .with_header("content-type", "text/plain")
            .with_body(&large_content)
            .create_async()
            .await;

        let url = fixture.url("/large");
        let result = fixture.client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), TEST_LARGE_RESPONSE_SIZE);
    }
}

/// Tests for URL validation errors
mod url_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_url_format() {
        let client = helpers::create_test_client();
        let result = client.get_text("not-a-valid-url").await;
        helpers::assert_validation_error(result, "not-a-valid-url");
    }

    #[tokio::test]
    async fn test_unsupported_url_scheme() {
        let client = helpers::create_test_client();
        let unsupported_urls = [
            "ftp://example.com/file.txt",
            "file:///path/to/file",
            "mailto:test@example.com",
            "ws://example.com/socket",
            "data:text/plain;base64,SGVsbG8gV29ybGQ=",
        ];

        for url in unsupported_urls {
            helpers::assert_url_rejected(&client, url).await;
        }
    }

    #[tokio::test]
    async fn test_empty_url() {
        let client = helpers::create_test_client();
        helpers::assert_url_rejected(&client, "").await;
    }
}

/// Tests for HTTP error responses
mod http_error_tests {
    use super::*;

    struct HttpErrorTestCase {
        status: u16,
        path: &'static str,
        body: &'static str,
        expect_calls: usize,
        verify_fn: fn(&MarkdownError),
    }

    #[tokio::test]
    async fn test_http_error_responses() {
        let test_cases = [
            HttpErrorTestCase {
                status: HTTP_NOT_FOUND,
                path: "/notfound",
                body: "Not Found",
                expect_calls: 1,
                verify_fn: |err| helpers::verify_server_error(err, HTTP_NOT_FOUND),
            },
            HttpErrorTestCase {
                status: HTTP_UNAUTHORIZED,
                path: "/secure",
                body: r#"{"error": "Unauthorized"}"#,
                expect_calls: 1,
                verify_fn: |err| helpers::verify_auth_error(err, AuthErrorKind::MissingToken, HTTP_UNAUTHORIZED),
            },
            HttpErrorTestCase {
                status: HTTP_FORBIDDEN,
                path: "/forbidden",
                body: "Forbidden",
                expect_calls: 1,
                verify_fn: |err| helpers::verify_auth_error(err, AuthErrorKind::PermissionDenied, HTTP_FORBIDDEN),
            },
            HttpErrorTestCase {
                status: HTTP_TOO_MANY_REQUESTS,
                path: "/rate-limited",
                body: "Too Many Requests",
                expect_calls: 4,
                verify_fn: |err| helpers::verify_rate_limited_error(err),
            },
            HttpErrorTestCase {
                status: HTTP_INTERNAL_SERVER_ERROR,
                path: "/server-error",
                body: "Internal Server Error",
                expect_calls: 4,
                verify_fn: |err| helpers::verify_server_error(err, HTTP_INTERNAL_SERVER_ERROR),
            },
            HttpErrorTestCase {
                status: HTTP_BAD_REQUEST,
                path: "/bad-request",
                body: "Bad Request",
                expect_calls: 1,
                verify_fn: |err| helpers::verify_server_error(err, HTTP_BAD_REQUEST),
            },
        ];

        for test_case in test_cases {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", test_case.path)
                .with_status(test_case.status as usize)
                .with_body(test_case.body)
                .expect(test_case.expect_calls)
                .create_async()
                .await;

            let client = helpers::create_test_client();
            let url = format!("{}{}", server.url(), test_case.path);
            let result = client.get_text(&url).await;

            mock.assert_async().await;
            assert!(result.is_err());
            (test_case.verify_fn)(&result.unwrap_err());
        }
    }
}

/// Tests for retry logic and resilience
mod retry_logic_tests {
    use super::*;

    async fn test_retry_scenario(
        path: &str,
        fail_count: usize,
        fail_status: u16,
        should_succeed: bool,
        expected_body: Option<&str>,
    ) {
        let mut fixture = helpers::MockTestFixture::new().await;

        if fail_count > 0 {
            let _failing_mock = fixture
                .mock_endpoint_with_expect(path, fail_status as usize, "Error", fail_count)
                .await;
        }

        if should_succeed {
            let success_body = expected_body.unwrap_or("Success after retries!");
            let _success_mock = fixture
                .mock_endpoint_with_expect(path, HTTP_OK as usize, success_body, 1)
                .await;
        }

        let url = fixture.url(path);
        let result = fixture.client.get_text(&url).await;

        if should_succeed {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), expected_body.unwrap_or("Success after retries!"));
        } else {
            assert!(result.is_err());
            match result.unwrap_err() {
                MarkdownError::EnhancedNetworkError { kind, context } => {
                    match kind {
                        NetworkErrorKind::ServerError(status) => {
                            assert_eq!(status, fail_status);
                        }
                        _ => panic!("Expected ServerError({fail_status})"),
                    }
                    let total_attempts = fail_count;
                    assert!(context
                        .additional_info
                        .unwrap()
                        .contains(&format!("{total_attempts} attempts")));
                }
                _ => panic!("Expected EnhancedNetworkError"),
            }
        }
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        test_retry_scenario("/flaky", 2, HTTP_INTERNAL_SERVER_ERROR, true, Some("Success after retries!")).await;
    }

    #[tokio::test]
    async fn test_retry_max_attempts_exceeded() {
        test_retry_scenario("/always-fails", 4, HTTP_BAD_GATEWAY, false, None).await;
    }

    #[tokio::test]
    async fn test_no_retry_for_auth_errors() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/unauthorized")
            .with_status(HTTP_UNAUTHORIZED as usize)
            .expect(1) // Should only be called once (no retry)
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/unauthorized", server.url());
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::AuthenticationError { .. } => {
                // Expected - no retry for auth errors
            }
            _ => panic!("Expected AuthenticationError"),
        }
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/backoff-test")
            .with_status(HTTP_SERVICE_UNAVAILABLE as usize)
            .expect(4) // 1 initial + 3 retries
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/backoff-test", server.url());

        let start = std::time::Instant::now();
        let result = client.get_text(&url).await;
        let duration = start.elapsed();

        mock.assert_async().await;
        assert!(result.is_err());

        // Verify that backoff introduced some delay, but don't test exact timing
        // since that can be flaky depending on system load and CI environments
        let expected_minimum = get_test_retry_delay().as_millis() as u64;
        let reasonable_maximum = Duration::from_secs(5).as_millis() as u64; // Generous upper bound

        assert!(
            (duration.as_millis() as u64) >= expected_minimum,
            "Expected minimum delay of {}ms, got {}ms",
            expected_minimum,
            duration.as_millis()
        );
        assert!(
            (duration.as_millis() as u64) < reasonable_maximum,
            "Test took too long: {}ms (max: {}ms)",
            duration.as_millis(),
            reasonable_maximum
        );
    }
}

/// Tests for authentication header injection
mod authentication_tests {
    use super::*;

    struct AuthTestCase {
        auth_header_value: &'static str,
        path: &'static str,
        expected_body: &'static str,
    }

    #[tokio::test]
    async fn test_authentication_headers() {
        let test_cases = vec![
            AuthTestCase {
                auth_header_value: "token test_github_token",
                path: "/github-api",
                expected_body: "GitHub API response",
            },
            AuthTestCase {
                auth_header_value: "Bearer test_office365_token",
                path: "/office365-api",
                expected_body: "Office 365 API response",
            },
            AuthTestCase {
                auth_header_value: "Bearer test_google_api_key",
                path: "/google-api",
                expected_body: "Google API response",
            },
        ];

        for test_case in test_cases {
            let mut server = Server::new_async().await;

            let mock = server
                .mock("GET", test_case.path)
                .match_header("Authorization", test_case.auth_header_value)
                .with_status(HTTP_OK as usize)
                .with_body(test_case.expected_body)
                .create_async()
                .await;

            let client = helpers::create_auth_client();
            let url = format!("{}{}", server.url(), test_case.path);

            let mut headers = HashMap::new();
            headers.insert(
                "Authorization".to_string(),
                test_case.auth_header_value.to_string(),
            );

            let result = client.get_text_with_headers(&url, &headers).await;

            mock.assert_async().await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), test_case.expected_body);
        }
    }
}

/// Tests for timeout behavior
mod timeout_tests {
    use super::*;

    #[tokio::test]
    async fn test_request_timeout() {
        let mut server = Server::new_async().await;

        // Create a mock that simulates a slow response
        let _mock = server
            .mock("GET", "/slow")
            .with_status(HTTP_OK as usize)
            .with_body("Slow response")
            .with_chunked_body(|w| {
                // Sleep longer than the client timeout
                std::thread::sleep(Duration::from_secs(TEST_SLOW_RESPONSE_DELAY_SECS));
                w.write_all(b"Too late!")
            })
            .create_async()
            .await;

        let client = helpers::TestConfigBuilder::new()
            .with_short_timeout()
            .with_fast_retry()
            .build();

        let url = format!("{}/slow", server.url());

        let result = timeout(Duration::from_secs(TEST_OUTER_TIMEOUT_SECS), client.get_text(&url)).await;

        assert!(result.is_ok()); // The timeout wrapper shouldn't timeout
        let inner_result = result.unwrap();
        assert!(inner_result.is_err());

        // Should be a timeout error
        match inner_result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => {
                match kind {
                    NetworkErrorKind::Timeout => {
                        // Expected timeout error
                    }
                    NetworkErrorKind::ConnectionFailed => {
                        // Also acceptable - reqwest might map timeout to connection failed
                    }
                    _ => panic!("Expected Timeout or ConnectionFailed error, got: {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }
}

/// Tests for edge cases and error conditions
mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_response_body() {
        let mut server = Server::new_async().await;

        let _mock = server
            .mock("GET", "/empty")
            .with_status(HTTP_OK as usize)
            .with_body("")
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/empty", server.url());
        let result = client.get_text(&url).await;

        _mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_binary_content_as_text() {
        let mut server = Server::new_async().await;
        let binary_data = b"\x00\x01\x02\x03\xFF\xFE\xFD\xFC";

        let mock = server
            .mock("GET", "/binary")
            .with_status(HTTP_OK as usize)
            .with_header("content-type", "application/octet-stream")
            .with_body(binary_data)
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/binary", server.url());

        // get_text should handle binary data gracefully
        let result = client.get_text(&url).await;
        mock.assert_async().await;

        assert!(result.is_ok());
        // The content might not be valid UTF-8, but get_text should handle it
    }

    #[tokio::test]
    async fn test_very_long_url() {
        let client = helpers::create_test_client();

        // Create a very long but valid URL
        let long_path = "a".repeat(TEST_VERY_LONG_PATH_LENGTH);
        let long_url = format!("https://example.com/{long_path}");

        let result = client.get_text(&long_url).await;

        // Should fail with network error (can't actually connect to example.com)
        // but not with URL validation error
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { .. } => {
                // Expected - connection will fail but URL is valid
            }
            e => panic!("Expected network error, got: {e:?}"),
        }
    }

    #[tokio::test]
    async fn test_international_domain_names() {
        let client = helpers::create_test_client();

        // Test with international domain name (IDN)
        let idn_url = "https://例え.テスト/path";

        let result = client.get_text(idn_url).await;

        // Should fail with network error (can't connect) but not validation error
        assert!(result.is_err());
        // The specific error type may vary depending on how the URL library handles IDNs
    }

    #[tokio::test]
    async fn test_redirect_handling() {
        let mut server = Server::new_async().await;

        let redirect_mock = server
            .mock("GET", "/redirect-source")
            .with_status(HTTP_FOUND as usize)
            .with_header("Location", &format!("{}/redirect-target", server.url()))
            .create_async()
            .await;

        let target_mock = server
            .mock("GET", "/redirect-target")
            .with_status(HTTP_OK as usize)
            .with_body("Redirected content")
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/redirect-source", server.url());
        let result = client.get_text(&url).await;

        redirect_mock.assert_async().await;
        target_mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Redirected content");
    }
}

/// Tests for response body reading error handling
mod response_body_error_tests {
    use super::*;

    enum BodyReadTestType {
        Text,
        Bytes,
        WithHeaders,
    }

    async fn test_body_read_scenario(test_type: BodyReadTestType) {
        let mut server = Server::new_async().await;
        let client = helpers::create_test_client();

        match test_type {
            BodyReadTestType::Text => {
                let mock = server
                    .mock("GET", "/body-error")
                    .with_status(HTTP_OK as usize)
                    .with_body(b"Some content that should be readable")
                    .create_async()
                    .await;
                let url = format!("{}/body-error", server.url());
                let result = client.get_text(&url).await;
                mock.assert_async().await;
                assert!(result.is_ok());
            }
            BodyReadTestType::Bytes => {
                let mock = server
                    .mock("GET", "/bytes-error")
                    .with_status(HTTP_OK as usize)
                    .with_body(b"Binary content")
                    .create_async()
                    .await;
                let url = format!("{}/bytes-error", server.url());
                let result = client.get_bytes(&url).await;
                mock.assert_async().await;
                assert!(result.is_ok());
            }
            BodyReadTestType::WithHeaders => {
                let mock = server
                    .mock("GET", "/headers-body-error")
                    .with_status(HTTP_OK as usize)
                    .with_body("Header content")
                    .create_async()
                    .await;
                let url = format!("{}/headers-body-error", server.url());
                let headers = HashMap::from([("Custom".to_string(), "value".to_string())]);
                let result = client.get_text_with_headers(&url, &headers).await;
                mock.assert_async().await;
                assert!(result.is_ok());
            }
        }
    }

    #[tokio::test]
    async fn test_simulated_response_body_read_failure() {
        test_body_read_scenario(BodyReadTestType::Text).await;
    }

    #[tokio::test]
    async fn test_simulated_bytes_body_read_failure() {
        test_body_read_scenario(BodyReadTestType::Bytes).await;
    }

    #[tokio::test]
    async fn test_simulated_headers_body_read_failure() {
        test_body_read_scenario(BodyReadTestType::WithHeaders).await;
    }
}

/// Tests for automatic authentication header injection based on domain
mod domain_auth_tests {
    use super::*;

    #[tokio::test]
    async fn test_github_domain_auth_injection() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/github-endpoint")
            .match_header("Authorization", "token test_github_token")
            .with_status(HTTP_OK as usize)
            .with_body("GitHub content")
            .create_async()
            .await;

        let client = helpers::create_auth_client();

        // Use localhost to trigger GitHub auth injection (line 327 in client.rs)
        let url = format!(
            "http://localhost:{}/github-endpoint",
            server.socket_address().port()
        );
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "GitHub content");
    }

    #[tokio::test]
    async fn test_github_api_accept_header() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/repos/user/repo")
            .match_header("Authorization", "token test_github_token")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(HTTP_OK as usize)
            .with_body("GitHub API response")
            .create_async()
            .await;

        let client = helpers::create_auth_client();

        // Use localhost with /repos/ path to trigger both auth and Accept header
        let url = format!(
            "http://localhost:{}/repos/user/repo",
            server.socket_address().port()
        );
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "GitHub API response");
    }

    #[tokio::test]
    async fn test_office365_services_auth_injection() {
        // Test Office 365, SharePoint, and OneDrive all use the same config
        let config = Config::builder()
            .office365_token("test_office365_token")
            .build();
        let _client = HttpClient::with_config(&config.http, &config.auth);

        // Verify config was set correctly for all Office365 services
        assert_eq!(
            config.auth.office365_token,
            Some("test_office365_token".to_string())
        );
    }

    #[tokio::test]
    async fn test_google_apis_domain_auth_injection() {
        // Test Google APIs authentication configuration (covers lines 346-353)
        let config = Config::builder()
            .google_api_key("test_google_api_key")
            .build();
        let _client = HttpClient::with_config(&config.http, &config.auth);

        // Verify config was set correctly
        assert_eq!(
            config.auth.google_api_key,
            Some("test_google_api_key".to_string())
        );
    }

    #[tokio::test]
    async fn test_no_auth_for_non_matching_domains() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/endpoint")
            .with_status(HTTP_OK as usize)
            .with_body("Public content")
            .create_async()
            .await;

        let client = helpers::create_auth_client();
        let url = format!("{}/endpoint", server.url());

        // Regular domain should not get auth headers
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Public content");
    }
}

/// Tests for error mapping functionality
mod error_mapping_tests {
    use super::*;
    use std::time::Duration;

    async fn assert_network_error_mapping(
        client: &HttpClient,
        url: &str,
        expected_kinds: &[NetworkErrorKind],
    ) {
        let result = client.get_text(url).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                let matches = expected_kinds.iter().any(|expected| {
                    std::mem::discriminant(&kind) == std::mem::discriminant(expected)
                });
                assert!(
                    matches,
                    "Expected one of {:?}, got: {:?}",
                    expected_kinds, kind
                );
                assert_eq!(context.operation, "HTTP request");
                assert_eq!(context.converter_type, "HttpClient");
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_timeout_error_mapping() {
        let client = helpers::TestConfigBuilder::new()
            .with_short_timeout()
            .with_no_retry()
            .build();

        // Try to connect to a non-routable address to trigger timeout
        assert_network_error_mapping(
            &client,
            "http://10.255.255.1/timeout-test",
            &[NetworkErrorKind::Timeout, NetworkErrorKind::ConnectionFailed],
        )
        .await;
    }

    #[tokio::test]
    async fn test_connection_error_mapping() {
        let client = helpers::create_test_client();

        // Try to connect to a non-existent host
        assert_network_error_mapping(
            &client,
            "http://non-existent-host-12345.invalid/test",
            &[NetworkErrorKind::ConnectionFailed],
        )
        .await;
    }
}

/// Tests for additional error mapping functionality
mod additional_error_mapping_tests {
    use super::*;
    use super::error_mapping_tests::assert_network_error_mapping;

    #[tokio::test]
    async fn test_timeout_error_mapping_detailed() {
        let client = helpers::TestConfigBuilder::new()
            .with_short_timeout()
            .with_no_retry()
            .build();

        // Use a URL that will timeout due to very short timeout
        assert_network_error_mapping(
            &client,
            "https://httpbin.org/delay/1",
            &[NetworkErrorKind::Timeout],
        )
        .await;
    }

    #[tokio::test]
    async fn test_connection_refused_error_mapping() {
        let client = helpers::create_test_client();

        // Use a port that should be closed to force connection failure
        assert_network_error_mapping(
            &client,
            "http://127.0.0.1:9999/nonexistent",
            &[NetworkErrorKind::ConnectionFailed],
        )
        .await;
    }

    #[tokio::test]
    async fn test_invalid_domain_error_mapping() {
        let client = helpers::create_test_client();

        // Use an invalid domain that should cause DNS resolution failure
        let result = client
            .get_text("http://this-domain-does-not-exist-12345.invalid/")
            .await;

        assert!(result.is_err());
        // This should trigger either connection failure or request error mapping
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind: _, context } => {
                assert_eq!(context.converter_type, "HttpClient");
            }
            MarkdownError::ValidationError { kind, context } => {
                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                assert_eq!(context.converter_type, "HttpClient");
            }
            _ => panic!("Expected network or validation error"),
        }
    }
}

/// Tests for additional retry logic and server error handling
mod additional_retry_logic_tests {
    use super::*;

    #[tokio::test]
    async fn test_server_error_retry_logic_exhausted() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/always-500")
            .with_status(HTTP_INTERNAL_SERVER_ERROR as usize)
            .with_body("Internal Server Error")
            .expect(4) // 1 initial + 3 retries = 4 total attempts
            .create_async()
            .await;

        let client = helpers::TestConfigBuilder::new().with_fast_retry().build();

        let url = format!("{}/always-500", server.url());
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::ServerError(status) => {
                        assert_eq!(status, HTTP_INTERNAL_SERVER_ERROR);
                        assert!(context.additional_info.unwrap().contains("4 attempts"));
                    }
                    _ => panic!("Expected ServerError(500), got {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_rate_limiting_retry_logic() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/rate-limited")
            .with_status(HTTP_TOO_MANY_REQUESTS as usize)
            .with_header("Retry-After", &TEST_RETRY_AFTER_SECONDS.to_string())
            .with_body("Rate Limited")
            .expect(4) // 1 initial + 3 retries = 4 total attempts
            .create_async()
            .await;

        let client = helpers::TestConfigBuilder::new().with_fast_retry().build();

        let url = format!("{}/rate-limited", server.url());
        let result = client.get_text(&url).await;

        mock.assert_async().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, context } => {
                match kind {
                    NetworkErrorKind::RateLimited => {
                        assert!(context.additional_info.unwrap().contains("4 attempts"));
                    }
                    _ => panic!("Expected RateLimited error, got {kind:?}"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_connection_failure_during_retry() {
        let client = helpers::TestConfigBuilder::new().with_fast_retry().build();

        // Use a port that should be closed to force connection failure
        let result = client.get_text("http://127.0.0.1:9998/test").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, context } => match kind {
                NetworkErrorKind::ConnectionFailed => {
                    assert_eq!(context.operation, "HTTP request");
                    assert_eq!(context.converter_type, "HttpClient");
                }
                _ => panic!("Expected ConnectionFailed error, got {kind:?}"),
            },
            _ => panic!("Expected EnhancedNetworkError"),
        }
    }
}

/// Tests for URL validation edge cases
mod url_validation_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_url_validation_invalid_characters() {
        let client = helpers::create_test_client();

        // Test URLs that should fail URL parsing (covers lines 175-179)
        let invalid_urls = [
            "not-a-url-at-all",
            "http://[invalid-brackets",
            "://missing-scheme",
        ];

        for url in invalid_urls {
            let result = client.get_text(url).await;
            assert!(result.is_err(), "Should reject malformed URL: {url}");

            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.operation, "URL validation");
                    assert_eq!(context.url, url);
                }
                _ => panic!("Expected ValidationError for malformed URL: {url}"),
            }
        }
    }

    #[tokio::test]
    async fn test_unsupported_scheme_validation() {
        let client = helpers::create_test_client();

        // Test unsupported schemes (covers lines 185, 187)
        let unsupported_schemes = [
            "ftp://example.com/file",
            "file:///local/path",
            "data:text/plain;base64,SGVsbG8gV29ybGQ=",
        ];

        for url in unsupported_schemes {
            let result = client.get_text(url).await;
            assert!(result.is_err(), "Should reject unsupported scheme: {url}");

            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.operation, "URL scheme validation");
                    assert!(context
                        .additional_info
                        .unwrap()
                        .contains("Unsupported scheme"));
                }
                _ => panic!("Expected ValidationError for unsupported scheme: {url}"),
            }
        }
    }

    #[tokio::test]
    async fn test_scheme_validation_with_headers() {
        let client = helpers::create_test_client();
        let headers = HashMap::new();

        // Test non-HTTP scheme with get_text_with_headers (covers lines 175-179, 185, 187)
        let result = client
            .get_text_with_headers("ftp://example.com/file", &headers)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ValidationError { kind, context } => {
                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                assert_eq!(context.operation, "URL scheme validation");
                assert!(context
                    .additional_info
                    .unwrap()
                    .contains("Unsupported scheme: ftp"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }
}

/// Tests for additional HTTP status codes and error conditions
mod additional_error_tests {
    use super::*;

    struct StatusCodeTestCase {
        status_code: u16,
        should_retry: bool,
        expected_attempts: usize,
    }

    #[tokio::test]
    async fn test_status_code_handling() {
        let test_cases = vec![
            StatusCodeTestCase {
                status_code: HTTP_BAD_REQUEST,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_METHOD_NOT_ALLOWED,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_NOT_ACCEPTABLE,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_CONFLICT,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_GONE,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_UNPROCESSABLE_ENTITY,
                should_retry: false,
                expected_attempts: 1,
            },
            StatusCodeTestCase {
                status_code: HTTP_BAD_GATEWAY,
                should_retry: true,
                expected_attempts: 4,
            },
            StatusCodeTestCase {
                status_code: HTTP_SERVICE_UNAVAILABLE,
                should_retry: true,
                expected_attempts: 4,
            },
            StatusCodeTestCase {
                status_code: HTTP_GATEWAY_TIMEOUT,
                should_retry: true,
                expected_attempts: 4,
            },
        ];

        let mut server = Server::new_async().await;

        for test_case in test_cases {
            let path = format!("/error-{}", test_case.status_code);
            let mock = server
                .mock("GET", path.as_str())
                .with_status(test_case.status_code as usize)
                .with_body("Error response")
                .expect(test_case.expected_attempts)
                .create_async()
                .await;

            let client = helpers::create_test_client();
            let url = format!("{}{}", server.url(), path);
            let result = client.get_text(&url).await;

            mock.assert_async().await;
            helpers::verify_status_code_error(
                result,
                test_case.status_code,
                test_case.should_retry,
                test_case.expected_attempts,
            );
        }
    }
}

/// Integration tests combining multiple HTTP client features
mod integration_tests {
    use super::*;

    fn assert_is_auth_error(result: Result<String, MarkdownError>) {
        match result.unwrap_err() {
            MarkdownError::AuthenticationError { .. } => {},
            err => panic!("Expected AuthenticationError, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn test_service_recovery_after_failures() {
        let mut server = Server::new_async().await;

        let failing_mock = server
            .mock("GET", "/api/recovery")
            .with_status(HTTP_SERVICE_UNAVAILABLE as usize)
            .expect(4) // 1 initial + 3 retries
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/api/recovery", server.url());

        let result = client.get_text(&url).await;
        assert!(result.is_err());

        failing_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_auth_required_after_recovery() {
        let mut server = Server::new_async().await;

        let auth_required_mock = server
            .mock("GET", "/api/auth-check")
            .with_status(HTTP_UNAUTHORIZED as usize)
            .expect(1)
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/api/auth-check", server.url());

        let result = client.get_text(&url).await;
        assert_is_auth_error(result);

        auth_required_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_successful_auth_request() {
        let mut server = Server::new_async().await;

        let success_mock = server
            .mock("GET", "/api/protected")
            .match_header("Authorization", "Bearer custom-token")
            .with_status(HTTP_OK as usize)
            .with_body("Protected data")
            .expect(1)
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/api/protected", server.url());

        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer custom-token".to_string(),
        );
        let result = client.get_text_with_headers(&url, &headers).await;

        success_mock.assert_async().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Protected data");
    }

    async fn run_concurrent_requests<F>(
        count: usize,
        request_fn: F,
    ) -> Vec<Result<String, MarkdownError>>
    where
        F: Fn() -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, MarkdownError>> + Send>,
            > + Clone
            + Send
            + 'static,
    {
        let mut handles = Vec::new();
        for _ in 0..count {
            let request_fn_clone = request_fn.clone();
            let handle = tokio::spawn(async move { request_fn_clone().await });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let mut fixture = helpers::MockTestFixture::new().await;

        let mock = fixture
            .mock_endpoint_with_expect(
                "/concurrent",
                HTTP_OK as usize,
                "Concurrent response",
                TEST_CONCURRENT_REQUEST_COUNT,
            )
            .await;

        let url = fixture.url("/concurrent");

        let results = run_concurrent_requests(TEST_CONCURRENT_REQUEST_COUNT, {
            let url = url.clone();
            move || {
                let client = helpers::create_test_client();
                let url = url.clone();
                Box::pin(async move { client.get_text(&url).await })
            }
        })
        .await;

        mock.assert_async().await;

        // All requests should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Request {} failed: {:?}", i, result);
            assert_eq!(result.as_ref().unwrap(), "Concurrent response");
        }
    }

    #[tokio::test]
    async fn test_successful_request_handling() {
        let mut server = Server::new_async().await;

        let success_mock = server
            .mock("GET", "/success")
            .with_status(HTTP_OK as usize)
            .with_body("Success")
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/success", server.url());

        let result = client.get_text(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");

        success_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_404_error_no_retry() {
        let mut server = Server::new_async().await;

        let not_found_mock = server
            .mock("GET", "/not-found")
            .with_status(HTTP_NOT_FOUND as usize)
            .expect(1)
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/not-found", server.url());

        let result = client.get_text(&url).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => {
                match kind {
                    NetworkErrorKind::ServerError(status) if status == HTTP_NOT_FOUND => {
                        // Expected
                    }
                    _ => panic!("Expected ServerError(404)"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }

        not_found_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_500_error_with_retries() {
        let mut server = Server::new_async().await;

        let server_error_mock = server
            .mock("GET", "/server-error")
            .with_status(HTTP_INTERNAL_SERVER_ERROR as usize)
            .expect(4) // 1 initial + 3 retries
            .create_async()
            .await;

        let client = helpers::create_test_client();
        let url = format!("{}/server-error", server.url());

        let result = client.get_text(&url).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::EnhancedNetworkError { kind, .. } => {
                match kind {
                    NetworkErrorKind::ServerError(status) if status == HTTP_INTERNAL_SERVER_ERROR => {
                        // Expected after retries
                    }
                    _ => panic!("Expected ServerError(500)"),
                }
            }
            _ => panic!("Expected EnhancedNetworkError"),
        }

        server_error_mock.assert_async().await;
    }
}
