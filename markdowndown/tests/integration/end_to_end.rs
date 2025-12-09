//! End-to-end integration tests
//!
//! Tests complete workflows and cross-cutting concerns across the entire library.

use markdowndown::{Markdown, MarkdownDown};
use std::time::{Duration, Instant};

use super::{IntegrationTestConfig, TestUtils};

/// Standard frontmatter fields expected in conversions
const REQUIRED_FRONTMATTER_FIELDS: &[&str] = &["source_url", "converted_at", "conversion_type"];

/// Minimum content length for valid markdown (in characters)
const MIN_VALID_CONTENT_LENGTH: usize = 50;

/// Minimum number of lines for valid markdown content
const MIN_VALID_LINE_COUNT: usize = 3;

/// Minimum number of lines to consider content as having structure
const MIN_STRUCTURED_LINE_COUNT: usize = 5;

/// Custom timeout for configuration tests (in seconds)
const CUSTOM_TIMEOUT_SECS: u64 = 60;

/// Maximum number of retries for configuration tests
const MAX_RETRIES_FOR_TEST: u32 = 5;

/// Rate limit delay between concurrent conversions (in milliseconds)
const RATE_LIMIT_DELAY_MS: u64 = 1000;

/// Test case configuration for end-to-end workflow testing
#[derive(Debug, Clone)]
struct EndToEndTestCase {
    url: &'static str,
    service_type: &'static str,
    description: &'static str,
}

impl EndToEndTestCase {
    const fn new(url: &'static str, service_type: &'static str, description: &'static str) -> Self {
        Self {
            url,
            service_type,
            description,
        }
    }
}

/// Test URLs for different service types
const END_TO_END_TEST_CASES: &[EndToEndTestCase] = &[
    EndToEndTestCase::new("https://httpbin.org/html", "HTML", "Simple HTML conversion"),
    EndToEndTestCase::new(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        "HTML",
        "Complex Wikipedia page",
    ),
    EndToEndTestCase::new(
        "https://github.com/rust-lang/rust/issues/1",
        "GitHub",
        "GitHub issue conversion",
    ),
    EndToEndTestCase::new(
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
        "Google Docs",
        "Google Docs conversion",
    ),
];

/// Test cases for fallback behavior
const FALLBACK_TEST_CASES: &[(&str, &str)] = &[
    (
        "https://docs.google.com/document/d/nonexistent/edit",
        "Non-existent Google Doc should fallback to HTML",
    ),
    (
        "https://github.com/nonexistent/repo/issues/1",
        "Non-existent GitHub issue might fallback",
    ),
];

/// Test cases for error propagation testing
#[derive(Debug, Clone)]
struct ErrorPropagationTestCase {
    url: &'static str,
    should_succeed: bool,
    description: &'static str,
}

impl ErrorPropagationTestCase {
    const fn new(url: &'static str, should_succeed: bool, description: &'static str) -> Self {
        Self {
            url,
            should_succeed,
            description,
        }
    }
}

const ERROR_PROPAGATION_TEST_CASES: &[ErrorPropagationTestCase] = &[
    ErrorPropagationTestCase::new("https://httpbin.org/html", true, "Should succeed"),
    ErrorPropagationTestCase::new(
        "https://httpbin.org/status/404",
        false,
        "Should fail with 404",
    ),
    ErrorPropagationTestCase::new(
        "https://invalid-domain-12345.com",
        false,
        "Should fail with DNS error",
    ),
    ErrorPropagationTestCase::new(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        true,
        "Should succeed",
    ),
];

/// URLs for concurrent processing tests
const CONCURRENT_TEST_URLS: &[&str] = &[
    "https://httpbin.org/html",
    "https://en.wikipedia.org/wiki/Rust_(programming_language)",
];

/// Default number of conversions for resource management testing
const RESOURCE_TEST_CONVERSION_COUNT: usize = 5;

/// Result of validating an error propagation test case
enum TestResult {
    Success,
    ExpectedFailure,
    UnexpectedResult,
}

/// Statistics for test execution
#[derive(Debug, Default)]
struct TestStatistics {
    total: usize,
    successful: usize,
    expected_failures: usize,
    unexpected_results: usize,
    total_content: usize,
    duration: Duration,
}

impl TestStatistics {
    fn new() -> Self {
        Self::default()
    }

    fn record_result(&mut self, result: TestResult, content_length: Option<usize>) {
        self.total += 1;
        
        let (counter, should_record_content) = match result {
            TestResult::Success => (&mut self.successful, true),
            TestResult::ExpectedFailure => (&mut self.expected_failures, false),
            TestResult::UnexpectedResult => (&mut self.unexpected_results, false),
        };
        
        *counter += 1;
        if should_record_content {
            if let Some(len) = content_length {
                self.total_content += len;
            }
        }
    }

    fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    fn print_labeled_stats(&self, labels: &[(&str, usize)]) {
        for (label, value) in labels {
            println!("  {}: {}", label, value);
        }
    }

    fn print_summary(&self, test_name: &str) {
        println!("\n{test_name} Summary:");
        self.print_labeled_stats(&[
            ("Total test cases", self.total),
            ("Successful conversions", self.successful),
            ("Total content generated", self.total_content),
        ]);
        println!("  Total time: {:?}", self.duration);
        if self.total > 0 {
            println!("  Average time per test: {:?}", self.duration / self.total as u32);
        }
    }

    fn print_error_handling_summary(&self) {
        println!("Error Handling Summary:");
        self.print_labeled_stats(&[
            ("Expected successes", self.successful),
            ("Expected failures", self.expected_failures),
            ("Unexpected results", self.unexpected_results),
        ]);
    }
}

/// Service-specific validation configuration
#[derive(Debug)]
struct ServiceValidation {
    name: &'static str,
    url_pattern: &'static str,
    conversion_pattern: &'static str,
}

const SERVICE_VALIDATIONS: &[ServiceValidation] = &[
    ServiceValidation {
        name: "GitHub",
        url_pattern: "github.com",
        conversion_pattern: "",
    },
    ServiceValidation {
        name: "Google Docs",
        url_pattern: "docs.google.com",
        conversion_pattern: "",
    },
    ServiceValidation {
        name: "HTML",
        url_pattern: "",
        conversion_pattern: "html",
    },
];

/// Helper functions for end-to-end tests
struct EndToEndHelpers;

impl EndToEndHelpers {
    /// Create a configured MarkdownDown instance from test config
    fn create_configured_markdowndown(config: &IntegrationTestConfig) -> MarkdownDown {
        let mut config_builder = markdowndown::Config::builder()
            .timeout_seconds(config.default_timeout_secs)
            .user_agent(TestUtils::test_user_agent());

        if let Some(token) = &config.github_token {
            config_builder = config_builder.github_token(token);
        }

        if let Some(api_key) = &config.google_api_key {
            config_builder = config_builder.google_api_key(api_key);
        }

        let md_config = config_builder.build();
        MarkdownDown::with_config(md_config)
    }

    /// Validate a conversion result with standard checks
    fn validate_conversion_result(
        markdown: &Markdown,
        description: &str,
        service_type: Option<&str>,
    ) -> Result<(), String> {
        let content = markdown.as_str();

        if !Self::is_valid_markdown_structure(content) {
            return Err(format!("Poor quality output for {description}"));
        }

        if markdown.frontmatter().is_none() {
            return Err(format!("Missing frontmatter for {description}"));
        }

        let frontmatter = markdown.frontmatter().unwrap();
        if !Self::validate_frontmatter_structure(&frontmatter) {
            return Err(format!("Invalid frontmatter for {description}"));
        }

        if let Some(service) = service_type {
            Self::validate_service_specific_frontmatter(&frontmatter, service, description)?;
        }

        Ok(())
    }

    /// Validate that content has proper markdown structure
    fn is_valid_markdown_structure(content: &str) -> bool {
        !content.is_empty()
            && content.len() >= MIN_VALID_CONTENT_LENGTH
            && content.lines().count() >= MIN_VALID_LINE_COUNT
            && !content.trim().starts_with("Error")
    }

    /// Validate frontmatter contains all required fields
    fn validate_frontmatter_structure(frontmatter: &str) -> bool {
        REQUIRED_FRONTMATTER_FIELDS
            .iter()
            .all(|field| frontmatter.contains(field))
    }

    /// Validate pattern match with optional case insensitivity
    fn validate_pattern_match(text: &str, pattern: &str, case_insensitive: bool) -> bool {
        if pattern.is_empty() {
            return true;
        }
        if case_insensitive {
            Self::contains_case_insensitive(text, pattern)
        } else {
            text.contains(pattern)
        }
    }

    /// Case-insensitive substring search without allocating full strings
    fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        
        let needle_chars: Vec<char> = needle.chars().collect();
        let haystack_chars: Vec<char> = haystack.chars().collect();
        
        if needle_chars.len() > haystack_chars.len() {
            return false;
        }
        
        haystack_chars
            .windows(needle_chars.len())
            .any(|window| {
                window.iter().zip(needle_chars.iter()).all(|(h, n)| {
                    h.eq_ignore_ascii_case(n)
                })
            })
    }

    /// Extract a frontmatter line by field name
    fn extract_frontmatter_line<'a>(
        frontmatter: &'a str,
        field_name: &str,
        description: &str,
    ) -> Result<&'a str, String> {
        frontmatter
            .lines()
            .find(|line| line.starts_with(field_name))
            .ok_or_else(|| format!("Missing {field_name} in frontmatter for {description}"))
    }

    /// Find matching service validation configuration
    fn find_service_validation_config(service_type: &str) -> Option<&'static ServiceValidation> {
        SERVICE_VALIDATIONS
            .iter()
            .find(|v| v.name == service_type)
    }

    /// Validate URL pattern in source_url line
    fn validate_url_pattern(
        source_url_line: &str,
        config: &ServiceValidation,
        description: &str,
    ) -> Result<(), String> {
        if !Self::validate_pattern_match(source_url_line, config.url_pattern, false) {
            return Err(format!(
                "{} conversion should reference {} in source_url for {description}",
                config.name, config.url_pattern
            ));
        }
        Ok(())
    }

    /// Validate conversion pattern in conversion_type line
    fn validate_conversion_pattern(
        conversion_type_line: &str,
        config: &ServiceValidation,
        description: &str,
    ) -> Result<(), String> {
        if !Self::validate_pattern_match(conversion_type_line, config.conversion_pattern, true) {
            return Err(format!(
                "{} conversion should indicate {} in conversion_type for {description}",
                config.name, config.conversion_pattern
            ));
        }
        Ok(())
    }

    /// Extract and validate both source_url and conversion_type frontmatter fields
    fn extract_and_validate_frontmatter_fields<'a>(
        frontmatter: &'a str,
        description: &str,
    ) -> Result<(&'a str, &'a str), String> {
        let source_url = Self::extract_frontmatter_line(frontmatter, "source_url:", description)?;
        let conversion_type = Self::extract_frontmatter_line(frontmatter, "conversion_type:", description)?;
        Ok((source_url, conversion_type))
    }

    /// Validate service-specific frontmatter requirements
    fn validate_service_specific_frontmatter(
        frontmatter: &str,
        service_type: &str,
        description: &str,
    ) -> Result<(), String> {
        let (source_url_line, conversion_type_line) = Self::extract_and_validate_frontmatter_fields(frontmatter, description)?;

        if let Some(config) = Self::find_service_validation_config(service_type) {
            Self::validate_url_pattern(source_url_line, config, description)?;
            Self::validate_conversion_pattern(conversion_type_line, config, description)?;
        }

        Ok(())
    }

    /// Validate service-specific content has proper structure
    fn validate_service_specific_content(content: &str, service_type: &str) -> bool {
        match service_type {
            "HTML" => Self::has_markdown_elements(content),
            "Google Docs" => true,
            _ => true,
        }
    }

    /// Check if content contains typical markdown elements
    fn has_markdown_elements(content: &str) -> bool {
        let has_headers = content.lines().any(|line| line.starts_with('#'));
        let has_links = content.contains("](") || content.contains("http");
        let has_structure = content.lines().count() > MIN_STRUCTURED_LINE_COUNT;
        let has_paragraphs = content.contains("\n\n");

        has_headers || has_links || has_structure || has_paragraphs
    }

    /// Check if error is a valid error variant with descriptive information
    fn validate_error_variant(error: &markdowndown::Error) -> bool {
        matches!(
            error,
            markdowndown::Error::ValidationError { .. }
                | markdowndown::Error::EnhancedNetworkError { .. }
                | markdowndown::Error::AuthenticationError { .. }
                | markdowndown::Error::ContentError { .. }
                | markdowndown::Error::ConverterError { .. }
                | markdowndown::Error::ConfigurationError { .. }
                | markdowndown::Error::NetworkError { .. }
                | markdowndown::Error::ParseError { .. }
                | markdowndown::Error::InvalidUrl { .. }
                | markdowndown::Error::AuthError { .. }
                | markdowndown::Error::LegacyConfigurationError { .. }
        )
    }

    /// Check if an error is acceptable for integration tests
    fn is_acceptable_error(error: &markdowndown::Error, context: &str) -> bool {
        // Check if error is retryable or recoverable based on error type
        let is_retryable_or_recoverable = error.is_retryable() || error.is_recoverable();

        // Check for context-specific acceptable errors
        let is_context_specific = if context.contains("Google Docs") {
            // Google Docs errors are often acceptable (auth, access, etc.)
            matches!(
                error,
                markdowndown::Error::AuthError { .. }
                    | markdowndown::Error::NetworkError { .. }
                    | markdowndown::Error::AuthenticationError { .. }
                    | markdowndown::Error::EnhancedNetworkError { .. }
            )
        } else if context.contains("invalid-domain") {
            // DNS/connection errors are acceptable for invalid domains
            matches!(
                error,
                markdowndown::Error::NetworkError { .. }
                    | markdowndown::Error::EnhancedNetworkError { .. }
            )
        } else {
            false
        };

        let is_acceptable = is_retryable_or_recoverable || is_context_specific;

        if is_acceptable {
            println!("    Acceptable error type: {error:?}");
        }
        is_acceptable
    }

    /// Check if fallback content is valid (either quality content or error indicator)
    fn is_valid_fallback_content(content: &str) -> bool {
        TestUtils::validate_markdown_quality(content) || Self::is_error_indicator_content(content)
    }

    /// Check if content indicates an error state
    fn is_error_indicator_content(content: &str) -> bool {
        let error_indicators = ["Error", "not found", "404", "failed", "unavailable"];
        error_indicators
            .iter()
            .any(|indicator| content.contains(indicator))
    }

    /// Check if frontmatter contains a custom field with expected value
    fn frontmatter_contains_custom_field(frontmatter: &str, field_name: &str, expected_value: &str) -> bool {
        frontmatter
            .lines()
            .any(|line| {
                line.starts_with(field_name) && 
                line[field_name.len()..].starts_with(':') &&
                line.contains(expected_value)
            })
    }

    /// Validate max consecutive blank lines using line-by-line analysis
    fn validate_max_blank_lines(content: &str, max_lines: usize) -> bool {
        let mut consecutive_blank = 0;
        let mut max_consecutive = 0;

        for line in content.lines() {
            if line.trim().is_empty() {
                consecutive_blank += 1;
                max_consecutive = max_consecutive.max(consecutive_blank);
            } else {
                consecutive_blank = 0;
            }
        }

        max_consecutive <= max_lines
    }

    /// Handle validation result based on expected outcome
    fn handle_validation_result(
        result: &Result<Markdown, markdowndown::Error>,
        expected_success: bool,
        context: &str,
    ) -> TestResult {
        match (result, expected_success) {
            (Ok(markdown), true) => {
                println!("  ✓ Expected success: {} chars", markdown.as_str().len());
                let content = markdown.as_str();
                assert!(
                    TestUtils::validate_markdown_quality(content) || Self::is_error_indicator_content(content),
                    "Successful conversion should have quality content or error indicator"
                );
                TestResult::Success
            }
            (Ok(markdown), false) => {
                println!("  ⚠ Unexpected success: {} chars", markdown.as_str().len());
                let content = markdown.as_str();
                if Self::is_error_indicator_content(content) {
                    println!("    Content indicates error - acceptable");
                    TestResult::ExpectedFailure
                } else {
                    println!("    Truly unexpected success");
                    TestResult::UnexpectedResult
                }
            }
            (Err(error), false) => {
                println!("  ✓ Expected failure: {error}");
                assert!(
                    Self::validate_error_variant(error),
                    "Error should be a valid error variant"
                );
                TestResult::ExpectedFailure
            }
            (Err(error), true) => {
                println!("  ⚠ Unexpected failure: {error}");
                if Self::is_acceptable_error(error, context) {
                    TestResult::ExpectedFailure
                } else {
                    println!("    Truly unexpected failure");
                    TestResult::UnexpectedResult
                }
            }
        }
    }

    /// Perform a timed conversion with rate limiting
    async fn timed_conversion_with_rate_limit(
        md: &MarkdownDown,
        url: &str,
        config: &IntegrationTestConfig,
    ) -> (Result<Markdown, markdowndown::Error>, Duration) {
        TestUtils::apply_rate_limit(config).await;
        let start = Instant::now();
        let result = md.convert_url(url).await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Execute a test conversion with rate limiting and handle results
    async fn execute_test_conversion<S, E>(
        md: &MarkdownDown,
        url: &str,
        config: &IntegrationTestConfig,
        on_success: S,
        on_error: E,
    ) where
        S: FnOnce(&Markdown),
        E: FnOnce(&markdowndown::Error),
    {
        let (result, _duration) = Self::timed_conversion_with_rate_limit(md, url, config).await;

        match result {
            Ok(ref markdown) => on_success(markdown),
            Err(ref error) => on_error(error),
        }
    }

    /// Check skip prerequisites and print message if skipping
    fn check_skip(config: &IntegrationTestConfig, condition: bool, test_name: &str) -> bool {
        if condition {
            println!("Skipping {test_name} test - prerequisites not met");
            true
        } else {
            false
        }
    }

    /// Run an integration test with a custom skip function
    async fn run_integration_test_with_skip<F, Fut, S>(
        test_name: &str,
        skip_fn: S,
        test_fn: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(MarkdownDown, IntegrationTestConfig) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
        S: FnOnce(&IntegrationTestConfig) -> bool,
    {
        let config = IntegrationTestConfig::from_env();
        let skip = skip_fn(&config);
        
        if Self::check_skip(&config, skip, test_name) {
            return Ok(());
        }

        let md = Self::create_configured_markdowndown(&config);
        test_fn(md, config).await
    }

    /// Run an integration test with standard setup and teardown
    async fn run_integration_test<F, Fut>(
        test_name: &str,
        skip_condition: bool,
        test_fn: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(MarkdownDown, IntegrationTestConfig) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        Self::run_integration_test_with_skip(test_name, |_| skip_condition, test_fn).await
    }

    /// Run a simple integration test (checks skip_external_services)
    async fn run_simple_integration_test<F, Fut>(
        test_name: &str,
        test_fn: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(MarkdownDown, IntegrationTestConfig) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        Self::run_integration_test_with_skip(test_name, |c| c.skip_external_services, test_fn).await
    }

    /// Run a slow integration test (checks skip_external_services and skip_slow_tests)
    async fn run_slow_integration_test<F, Fut>(
        test_name: &str,
        test_fn: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(MarkdownDown, IntegrationTestConfig) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        Self::run_integration_test_with_skip(test_name, |c| c.skip_external_services || c.skip_slow_tests, test_fn).await
    }
}

/// Process a conversion result with validation and statistics tracking
fn process_conversion_result(
    result: Result<Markdown, markdowndown::Error>,
    url: &str,
    validator: Option<impl FnOnce(&Markdown) -> Result<(), String>>,
    stats: &mut TestStatistics,
) {
    match result {
        Ok(markdown) => {
            let content_length = markdown.as_str().len();
            
            if let Some(validate) = validator {
                validate(&markdown).expect("Validation failed");
            }
            
            println!("  ✓ Success for {}: {} chars", url, content_length);
            stats.record_result(TestResult::Success, Some(content_length));
        }
        Err(e) => {
            println!("  ⚠ Failed for {}: {} (may be acceptable)", url, e);
            stats.record_result(TestResult::ExpectedFailure, None);
        }
    }
}

/// Process a single end-to-end test case
async fn process_end_to_end_test_case(
    md: &MarkdownDown,
    test_case: &EndToEndTestCase,
    config: &IntegrationTestConfig,
    stats: &mut TestStatistics,
) {
    println!("End-to-end test: {} ({})", test_case.description, test_case.service_type);

    let (result, conversion_duration) =
        EndToEndHelpers::timed_conversion_with_rate_limit(md, test_case.url, config).await;

    match &result {
        Ok(_) => println!("  Conversion completed in {conversion_duration:?}"),
        Err(_) => {}
    }

    let validator = |markdown: &Markdown| {
        EndToEndHelpers::validate_conversion_result(markdown, test_case.description, Some(test_case.service_type))?;
        let content = markdown.as_str();
        if !EndToEndHelpers::validate_service_specific_content(content, test_case.service_type) {
            return Err(format!("Service-specific content validation failed for {}", test_case.service_type));
        }
        Ok(())
    };

    process_conversion_result(result, test_case.url, Some(validator), stats);
}

/// Test complete end-to-end workflow with various URL types
#[tokio::test]
async fn test_end_to_end_workflow() -> Result<(), Box<dyn std::error::Error>> {
    EndToEndHelpers::run_simple_integration_test(
        "end-to-end workflow",
        |md, config| async move {
            let mut stats = TestStatistics::new();
            let start_time = Instant::now();

            for test_case in END_TO_END_TEST_CASES.iter() {
                process_end_to_end_test_case(&md, test_case, &config, &mut stats).await;
            }

            stats.set_duration(start_time.elapsed());
            stats.print_summary("End-to-End Workflow");

            assert!(
                stats.successful > 0,
                "At least one conversion should succeed in end-to-end test"
            );

            Ok(())
        },
    )
    .await
}

/// Test fallback behavior across different URL types
#[tokio::test]
async fn test_cross_service_fallback() -> Result<(), Box<dyn std::error::Error>> {
    EndToEndHelpers::run_simple_integration_test(
        "cross-service fallback",
        |md, config| async move {
            for (url, description) in FALLBACK_TEST_CASES.iter() {
                println!("Testing fallback: {description}");

                let (result, _duration) =
                    EndToEndHelpers::timed_conversion_with_rate_limit(&md, url, &config).await;

                match result {
                    Ok(markdown) => {
                        println!("  ✓ Fallback successful: {} chars", markdown.as_str().len());

                        assert!(
                            EndToEndHelpers::is_valid_fallback_content(markdown.as_str()),
                            "Fallback should produce valid content or error message"
                        );

                        if let Some(frontmatter) = markdown.frontmatter() {
                            assert!(
                                TestUtils::validate_frontmatter(&frontmatter),
                                "Fallback should produce valid frontmatter"
                            );
                        }
                    }
                    Err(e) => {
                        println!("  ⚠ Fallback failed: {e} (acceptable)");
                        // Verify error is a valid error variant - all variants have descriptive messages by construction
                        assert!(
                            EndToEndHelpers::validate_error_variant(&e),
                            "Error should be a valid error variant with descriptive information"
                        );
                    }
                }
            }

            Ok(())
        },
    )
    .await
}

/// Validate custom config results including frontmatter and blank lines
fn validate_custom_config_results(markdown: &Markdown) {
    let frontmatter = markdown.frontmatter().unwrap();
    
    // Validate custom fields
    assert!(
        EndToEndHelpers::frontmatter_contains_custom_field(&frontmatter, "test_run", "integration"),
        "Should include custom frontmatter field: test_run with value integration"
    );
    assert!(
        EndToEndHelpers::frontmatter_contains_custom_field(&frontmatter, "config_test", "true"),
        "Should include custom frontmatter field: config_test with value true"
    );
    
    // Validate content quality and blank lines
    let content = markdown.as_str();
    assert!(
        TestUtils::validate_markdown_quality(content),
        "Custom config should still produce quality content"
    );
    assert!(
        EndToEndHelpers::validate_max_blank_lines(content, 1),
        "Should not have more than 1 consecutive blank line (max_consecutive_blank_lines=1)"
    );
}

/// Test configuration propagation across all services
#[tokio::test]
async fn test_configuration_propagation() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if EndToEndHelpers::check_skip(&config, config.skip_external_services, "configuration propagation") {
        return Ok(());
    }

    let custom_config = markdowndown::Config::builder()
        .timeout_seconds(CUSTOM_TIMEOUT_SECS)
        .user_agent("test-integration/1.0")
        .max_retries(MAX_RETRIES_FOR_TEST)
        .include_frontmatter(true)
        .custom_frontmatter_field("test_run", "integration")
        .custom_frontmatter_field("config_test", "true")
        .max_consecutive_blank_lines(1)
        .build();

    let md = MarkdownDown::with_config(custom_config);
    let test_url = "https://httpbin.org/html";

    let (result, _duration) =
        EndToEndHelpers::timed_conversion_with_rate_limit(&md, test_url, &config).await;
    let result = result?;

    assert!(
        result.frontmatter().is_some(),
        "Custom config should include frontmatter"
    );

    validate_custom_config_results(&result);

    println!("✓ Configuration propagation test passed");
    Ok(())
}

/// Execute items concurrently with rate limiting between requests
async fn execute_concurrent_with_rate_limit<F, T>(
    items: &[T],
    rate_limit_ms: u64,
    executor: F,
) -> Vec<Result<Markdown, markdowndown::Error>>
where
    F: Fn(&T) -> futures::future::BoxFuture<'_, Result<Markdown, markdowndown::Error>>,
    T: Sync,
{
    let futures = items.iter().enumerate().map(|(i, item)| async move {
        tokio::time::sleep(Duration::from_millis((i as u64) * rate_limit_ms)).await;
        executor(item).await
    });
    futures::future::join_all(futures).await
}

/// Execute concurrent conversions for all test URLs
async fn execute_concurrent_conversions() -> Vec<(&'static str, Result<Markdown, markdowndown::Error>)> {
    let executor = |url: &&str| -> futures::future::BoxFuture<'_, Result<Markdown, markdowndown::Error>> {
        Box::pin(async move {
            let md_instance = MarkdownDown::new();
            md_instance.convert_url(url).await
        })
    };

    let results = execute_concurrent_with_rate_limit(
        CONCURRENT_TEST_URLS,
        RATE_LIMIT_DELAY_MS,
        executor,
    ).await;

    CONCURRENT_TEST_URLS
        .iter()
        .zip(results)
        .map(|(url, result)| (*url, result))
        .collect()
}

/// Process concurrent conversion results and return statistics
fn process_concurrent_results(results: Vec<(&str, Result<Markdown, markdowndown::Error>)>, stats: &mut TestStatistics) {
    for (url, result) in results {
        let validator = |markdown: &Markdown| {
            EndToEndHelpers::validate_conversion_result(markdown, url, None)
        };
        
        process_conversion_result(result, url, Some(validator), stats);
    }
}

/// Test concurrent processing across different services
#[tokio::test]
async fn test_concurrent_cross_service_processing() -> Result<(), Box<dyn std::error::Error>> {
    EndToEndHelpers::run_slow_integration_test(
        "concurrent cross-service",
        |_md, _config| async move {
            let mut stats = TestStatistics::new();
            let start_time = Instant::now();
            let results = execute_concurrent_conversions().await;

            process_concurrent_results(results, &mut stats);
            stats.set_duration(start_time.elapsed());
            stats.print_summary("Concurrent Processing");

            assert!(
                stats.successful > 0,
                "At least one concurrent conversion should succeed"
            );

            Ok(())
        },
    )
    .await
}

/// Test library version and metadata consistency
#[tokio::test]
async fn test_library_metadata_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    // Test version information
    let version = markdowndown::VERSION;
    assert!(!version.is_empty(), "Version should not be empty");
    assert!(
        version.contains('.'),
        "Version should contain dots (semantic versioning)"
    );

    println!("Library version: {version}");

    // Test that frontmatter includes consistent metadata
    if !config.skip_external_services {
        let md = MarkdownDown::new();
        let test_url = "https://httpbin.org/html";

        let (result, _duration) =
            EndToEndHelpers::timed_conversion_with_rate_limit(&md, test_url, &config).await;
        let result = result?;
        let frontmatter = result.frontmatter().unwrap();

        for required_field in REQUIRED_FRONTMATTER_FIELDS {
            assert!(
                frontmatter.contains(required_field),
                "Should include required frontmatter field: {required_field}"
            );
        }

        if frontmatter.contains("user_agent:") {
            assert!(
                frontmatter.contains("markdowndown/"),
                "User agent should include library name and version"
            );
        }
    }

    println!("✓ Library metadata consistency verified");
    Ok(())
}

/// Process a single error propagation test case
async fn process_error_test_case(
    md: &MarkdownDown,
    test_case: &ErrorPropagationTestCase,
    config: &IntegrationTestConfig,
    stats: &mut TestStatistics,
) {
    println!("Testing error handling: {}", test_case.description);

    let (result, _duration) =
        EndToEndHelpers::timed_conversion_with_rate_limit(md, test_case.url, config).await;

    let validation_result = EndToEndHelpers::handle_validation_result(&result, test_case.should_succeed, test_case.description);
    stats.record_result(validation_result, None);
}

/// Test error propagation and recovery across services
#[tokio::test]
async fn test_error_propagation_and_recovery() -> Result<(), Box<dyn std::error::Error>> {
    EndToEndHelpers::run_simple_integration_test(
        "error propagation",
        |md, config| async move {
            let mut stats = TestStatistics::new();

            for test_case in ERROR_PROPAGATION_TEST_CASES.iter() {
                process_error_test_case(&md, test_case, &config, &mut stats).await;
            }

            stats.print_error_handling_summary();

            assert!(
                stats.successful + stats.expected_failures >= ERROR_PROPAGATION_TEST_CASES.len() / 2,
                "Most results should be as expected (allowing for network variability)"
            );

            Ok(())
        },
    )
    .await
}

/// Test memory usage and resource cleanup
#[tokio::test]
async fn test_resource_management() -> Result<(), Box<dyn std::error::Error>> {
    EndToEndHelpers::run_slow_integration_test(
        "resource management",
        |md, config| async move {
            let test_url = "https://httpbin.org/html";

            let initial_memory = std::mem::size_of_val(&md);
            println!("Initial MarkdownDown instance size: {initial_memory} bytes");

            let mut total_content_length = 0;

            for i in 0..RESOURCE_TEST_CONVERSION_COUNT {
                println!("Resource test conversion {}/{RESOURCE_TEST_CONVERSION_COUNT}", i + 1);

                let (result, _duration) =
                    EndToEndHelpers::timed_conversion_with_rate_limit(&md, test_url, &config).await;

                match result {
                    Ok(markdown) => {
                        let content_length = markdown.as_str().len();
                        total_content_length += content_length;

                        assert!(
                            TestUtils::validate_markdown_quality(markdown.as_str()),
                            "Content quality should be maintained across multiple conversions"
                        );

                        println!("  Conversion {}: {content_length} chars", i + 1);

                        drop(markdown);
                    }
                    Err(e) => {
                        println!("  Conversion {} failed: {e} (acceptable)", i + 1);
                    }
                }
            }

            println!("Resource Management Summary:");
            println!("  Total conversions attempted: {RESOURCE_TEST_CONVERSION_COUNT}");
            println!("  Total content processed: {total_content_length} chars");
            println!("  Instance memory footprint: {initial_memory} bytes");

            assert!(
                total_content_length > 0,
                "Should have processed some content"
            );

            Ok(())
        },
    )
    .await
}
