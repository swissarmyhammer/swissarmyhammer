//! Integration tests for GitHub issues and pull requests conversion
//!
//! Tests the library's ability to convert GitHub issues and PRs to markdown.

use markdowndown::{types::Markdown, MarkdownDown};
use std::time::Instant;

use super::{IntegrationTestConfig, TestUrls, TestUtils};

/// Test thresholds for validation
struct TestThresholds {
    min_issue_content_length: usize,
    min_pr_content_length: usize,
    min_error_message_length: usize,
}

impl Default for TestThresholds {
    fn default() -> Self {
        Self {
            min_issue_content_length: 100,
            min_pr_content_length: 50,
            min_error_message_length: 10,
        }
    }
}

impl TestThresholds {
    fn instance() -> Self {
        Self::default()
    }
}

// Rate limiting configuration
const RATE_LIMIT_DELAY_MS: u64 = 500;

// Test data constants - GitHub issue and PR numbers for testing
const TEST_GITHUB_PR_NUMBER: u32 = 2000;
const TEST_ISSUE_NUMBER_1: u32 = 12345;
const TEST_PR_NUMBER_1: u32 = 67890;
const TEST_API_ISSUE_NUMBER: u32 = 123;
const NONEXISTENT_ISSUE_NUMBER: u32 = 999999;
const NONEXISTENT_PR_NUMBER: u32 = 999999;
const TEST_TOKIO_ISSUE_NUMBER: u32 = 1000;

/// Create a MarkdownDown instance configured with the GitHub token from config.
fn create_github_markdown_instance(token: &str) -> MarkdownDown {
    let github_config = markdowndown::Config::builder().github_token(token).build();
    MarkdownDown::with_config(github_config)
}

/// Apply rate limiting and convert a URL.
async fn convert_with_rate_limit(
    md: &MarkdownDown,
    url: &str,
    config: &IntegrationTestConfig,
) -> Result<Markdown, markdowndown::Error> {
    TestUtils::apply_rate_limit(config).await;
    md.convert_url(url).await
}

/// Validate GitHub-specific frontmatter content.
fn validate_github_frontmatter(
    markdown: &Markdown,
    description: &str,
    expected_fragments: &[&str],
) {
    let frontmatter = markdown
        .frontmatter()
        .expect(&format!("Missing frontmatter for {description}"));

    assert!(
        TestUtils::validate_frontmatter(&frontmatter),
        "Invalid frontmatter for {description}"
    );

    assert!(
        frontmatter.contains("github.com"),
        "Should reference GitHub in frontmatter"
    );

    for fragment in expected_fragments {
        assert!(
            frontmatter.contains(fragment),
            "Frontmatter should contain '{fragment}' for {description}"
        );
    }
}

/// Check if an error is an acceptable GitHub error.
fn is_acceptable_github_error_type(error: &markdowndown::Error) -> bool {
    error.is_rate_limit() || error.is_forbidden() || error.is_unauthorized() || error.is_not_found()
}

/// Validate markdown content quality with standard checks.
fn validate_markdown_content(content: &str, context: &str) -> Result<(), String> {
    if !TestUtils::validate_markdown_quality(content) {
        return Err(format!("Poor quality markdown for {context}: content too short or invalid"));
    }
    
    if !content.contains('#') {
        return Err(format!("Should have headers for {context}"));
    }
    
    Ok(())
}

/// Generic validation and logging function that handles both success and error cases using Result.
fn validate_and_log_result(result: Result<&str, &markdowndown::Error>, context: &str) {
    match result {
        Ok(content) => {
            validate_markdown_content(content, context).expect("Validation failed");
            println!("✓ {context} validation passed ({} chars)", content.len());
        }
        Err(error) => {
            println!("⚠ {context} failed: {error}");
            assert!(
                is_acceptable_github_error_type(error),
                "Error should be an acceptable GitHub error type (rate limit, forbidden, unauthorized, or not found)"
            );
        }
    }
}

/// Result of a conversion operation with timing and metrics.
struct ConversionResult {
    duration: std::time::Duration,
    content_length: usize,
    success: bool,
}

/// Generic conversion handler with timing and validation.
/// Consolidates common conversion patterns used throughout the tests.
async fn handle_conversion_with_timing<V>(
    md: &MarkdownDown,
    url: &str,
    config: &IntegrationTestConfig,
    validator: V,
) -> Result<ConversionResult, Box<dyn std::error::Error>>
where
    V: FnOnce(&str) -> Result<(), String>,
{
    let start = Instant::now();
    let result = convert_with_rate_limit(md, url, config).await;
    let duration = start.elapsed();
    
    match result {
        Ok(markdown) => {
            let content = markdown.as_str();
            validator(content)?;
            validate_markdown_content(content, url)?;
            
            if duration >= config.default_timeout() {
                return Err(format!("Conversion took too long for {url}: {duration:?}").into());
            }
            
            println!("  Duration: {duration:?}");
            Ok(ConversionResult {
                duration,
                content_length: content.len(),
                success: true,
            })
        }
        Err(e) if is_acceptable_github_error_type(&e) => {
            Ok(ConversionResult {
                duration,
                content_length: 0,
                success: false,
            })
        }
        Err(e) => Err(e.into()),
    }
}

/// Convert and handle a GitHub URL with comprehensive validation.
/// This consolidates conversion, error handling, and validation into a single function.
async fn convert_and_handle_github_url(
    md: &MarkdownDown,
    url: &str,
    config: &IntegrationTestConfig,
    min_length: usize,
    additional_validation: impl FnOnce(&str),
) -> Result<(), Box<dyn std::error::Error>> {
    let result = handle_conversion_with_timing(md, url, config, |content| {
        if content.len() <= min_length {
            return Err(format!("Content should have substantial length (min: {min_length})"));
        }
        additional_validation(content);
        Ok(())
    }).await?;

    if result.success {
        println!("✓ {url} converted successfully ({} chars)", result.content_length);
    } else {
        println!("⚠ {url} failed - acceptable in testing");
    }
    
    Ok(())
}

/// GitHub test context that encapsulates setup and configuration.
struct GithubTestContext {
    md: MarkdownDown,
    config: IntegrationTestConfig,
}

impl GithubTestContext {
    /// Setup GitHub test context, returning an error if tests should be skipped.
    fn setup() -> Result<Self, &'static str> {
        let config = IntegrationTestConfig::from_env();
        if !config.can_test_github() {
            return Err("GitHub tests disabled - no token available or external services disabled");
        }
        let md = create_github_markdown_instance(config.github_token.as_ref().unwrap());
        Ok(Self { md, config })
    }
}

/// Test a single GitHub URL conversion with validation.
async fn test_single_github_url(
    md: &MarkdownDown,
    url: &str,
    description: &str,
    config: &IntegrationTestConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let thresholds = TestThresholds::instance();
    println!("Testing: {description} - {url}");
    convert_and_handle_github_url(md, url, config, thresholds.min_issue_content_length, |_| {}).await
}

/// Test conversion of GitHub issues and pull requests
#[tokio::test]
async fn test_github_conversions() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = match GithubTestContext::setup() {
        Ok(ctx) => ctx,
        Err(msg) => {
            println!("Skipping GitHub tests - {msg}");
            return Ok(());
        }
    };

    for (url, description) in TestUrls::GITHUB_TEST_URLS.iter() {
        test_single_github_url(&ctx.md, url, description, &ctx.config).await?;
    }

    Ok(())
}

/// Helper function to test GitHub resource conversion (issues or pull requests)
async fn test_github_resource_conversion(
    url: &str,
    min_content_length: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = match GithubTestContext::setup() {
        Ok(ctx) => ctx,
        Err(msg) => {
            println!("Skipping GitHub test - {msg}");
            return Ok(());
        }
    };

    convert_and_handle_github_url(&ctx.md, url, &ctx.config, min_content_length, |_| {}).await
}

/// Test specific GitHub issue and pull request conversions
#[tokio::test]
async fn test_github_issue_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let thresholds = TestThresholds::instance();
    
    let test_cases = [
        ("https://github.com/rust-lang/rust/issues/1", thresholds.min_issue_content_length),
        (&format!("https://github.com/serde-rs/serde/pull/{}", TEST_GITHUB_PR_NUMBER), thresholds.min_pr_content_length),
    ];
    
    for (url, min_length) in test_cases.iter() {
        test_github_resource_conversion(url, *min_length).await?;
    }
    
    Ok(())
}

/// Test GitHub pull request conversion
#[tokio::test]
async fn test_github_pull_request_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let thresholds = TestThresholds::instance();
    test_github_resource_conversion(
        &format!("https://github.com/serde-rs/serde/pull/{}", TEST_GITHUB_PR_NUMBER),
        thresholds.min_pr_content_length,
    )
    .await
}

/// Test GitHub URL format detection
#[tokio::test]
async fn test_github_url_detection() -> Result<(), Box<dyn std::error::Error>> {
    let _config = IntegrationTestConfig::from_env();

    // Test URL detection (doesn't require token)
    let github_urls = [
        &format!("https://github.com/rust-lang/rust/issues/{}", TEST_ISSUE_NUMBER_1),
        &format!("https://github.com/microsoft/vscode/pull/{}", TEST_PR_NUMBER_1),
        "https://github.com/facebook/react/issues/1",
        &format!("https://api.github.com/repos/owner/repo/issues/{}", TEST_API_ISSUE_NUMBER),
    ];

    for url in github_urls.iter() {
        println!("Testing URL detection: {url}");

        let detected_type = markdowndown::detect_url_type(url.as_ref())?;
        assert_eq!(
            detected_type,
            markdowndown::types::UrlType::GitHubIssue,
            "Should detect as GitHub issue/PR: {url}"
        );
    }

    println!("✓ All GitHub URL formats detected correctly");
    Ok(())
}

/// Result of a rate-limited request attempt.
struct RateLimitRequestResult {
    success: bool,
    rate_limited: bool,
}

/// Make a single rate-limited request and classify the result.
async fn make_rate_limited_request(
    md: &MarkdownDown,
    url: &str,
    request_num: usize,
    config: &IntegrationTestConfig,
) -> RateLimitRequestResult {
    println!("Request {request_num}: {url}");

    if request_num > 1 {
        tokio::time::sleep(std::time::Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;
    }

    let result = convert_with_rate_limit(md, url, config).await;

    match result {
        Ok(markdown) => {
            println!("  ✓ Success ({} chars)", markdown.as_str().len());
            assert!(TestUtils::validate_markdown_quality(markdown.as_str()));
            RateLimitRequestResult {
                success: true,
                rate_limited: false,
            }
        }
        Err(e) => {
            println!("  ⚠ Failed: {e}");
            let rate_limited = is_acceptable_github_error_type(&e);
            if rate_limited {
                println!("    Rate limited - this is expected behavior");
            } else {
                println!("    Unexpected error: {e}");
            }
            RateLimitRequestResult {
                success: false,
                rate_limited,
            }
        }
    }
}

/// Process the result of a single rate-limited request and update counters.
fn process_rate_limit_result(result: &RateLimitRequestResult, successes: &mut usize, rate_limited: &mut usize) {
    if result.success {
        *successes += 1;
    }
    if result.rate_limited {
        *rate_limited += 1;
    }
}

/// Execute rate-limited requests and return counts of successes and rate-limited responses.
async fn execute_rate_limit_requests(
    md: &MarkdownDown,
    urls: &[&str],
    config: &IntegrationTestConfig,
) -> (usize, usize, std::time::Duration) {
    let mut successes = 0;
    let mut rate_limited = 0;
    let start = Instant::now();

    for (i, url) in urls.iter().enumerate() {
        let result = make_rate_limited_request(md, url, i + 1, config).await;
        process_rate_limit_result(&result, &mut successes, &mut rate_limited);
    }

    let duration = start.elapsed();
    (successes, rate_limited, duration)
}

/// Test GitHub rate limiting behavior
#[tokio::test]
async fn test_github_rate_limiting() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = match GithubTestContext::setup() {
        Ok(ctx) => ctx,
        Err(msg) => {
            println!("Skipping GitHub rate limiting test - {msg}");
            return Ok(());
        }
    };

    if ctx.config.skip_slow_tests {
        println!("Skipping GitHub rate limiting test - slow tests skipped");
        return Ok(());
    }

    let urls = [
        "https://github.com/rust-lang/rust/issues/1",
        "https://github.com/rust-lang/rust/issues/2",
        "https://github.com/rust-lang/rust/issues/3",
    ];

    let (successes, rate_limited, duration) = execute_rate_limit_requests(&ctx.md, &urls, &ctx.config).await;

    println!("Rate limiting test results:");
    println!("  Total requests: {}", urls.len());
    println!("  Successes: {successes}");
    println!("  Rate limited: {rate_limited}");
    println!("  Duration: {duration:?}");

    assert!(
        successes + rate_limited == urls.len(),
        "All requests should either succeed or be rate limited"
    );

    Ok(())
}

/// Authentication test outcome classification.
enum AuthTestOutcome {
    BothSucceeded(Markdown, Markdown),
    BothFailed(markdowndown::Error, markdowndown::Error),
    NoTokenSucceeded(Markdown, markdowndown::Error),
    WithTokenSucceeded(markdowndown::Error, Markdown),
}

/// Classify authentication test results into outcome categories using tuple matching.
fn classify_auth_results(
    result_no_token: Result<Markdown, markdowndown::Error>,
    result_with_token: Result<Markdown, markdowndown::Error>,
) -> AuthTestOutcome {
    match (result_no_token, result_with_token) {
        (Ok(content1), Ok(content2)) => AuthTestOutcome::BothSucceeded(content1, content2),
        (Ok(content), Err(e)) => AuthTestOutcome::NoTokenSucceeded(content, e),
        (Err(e), Ok(content)) => AuthTestOutcome::WithTokenSucceeded(e, content),
        (Err(e1), Err(e2)) => AuthTestOutcome::BothFailed(e1, e2),
    }
}

/// Implementation of reporting methods for AuthTestOutcome.
impl AuthTestOutcome {
    /// Report the authentication test outcome with appropriate logging and validation.
    fn report(&self) {
        match self {
            Self::BothSucceeded(content1, content2) => {
                println!("Both conversions succeeded");
                validate_and_log_result(Ok(content1.as_str()), "No token");
                validate_and_log_result(Ok(content2.as_str()), "With token");
                println!("  No token: {} chars", content1.as_str().len());
                println!("  With token: {} chars", content2.as_str().len());
            }
            Self::BothFailed(e1, e2) => {
                println!("Both conversions failed");
                println!("  No token error: {e1}");
                println!("  With token error: {e2}");
            }
            Self::NoTokenSucceeded(content, error) => {
                println!("No-token succeeded, with-token failed: {error}");
                validate_and_log_result(Ok(content.as_str()), "No token conversion");
            }
            Self::WithTokenSucceeded(error, content) => {
                println!("No-token failed, with-token succeeded: {error}");
                validate_and_log_result(Ok(content.as_str()), "With token conversion");
            }
        }
    }
}

/// Compare authentication results from conversions with and without tokens.
fn compare_auth_results(
    result_no_token: Result<Markdown, markdowndown::Error>,
    result_with_token: Result<Markdown, markdowndown::Error>,
) {
    let outcome = classify_auth_results(result_no_token, result_with_token);
    outcome.report();
}

/// Check if an error is authentication-related.
fn is_auth_related_error(error: &markdowndown::Error) -> bool {
    error.is_forbidden() || error.is_unauthorized()
}

/// Handle authentication test result for no-token scenario.
fn handle_no_token_result(result: Result<Markdown, markdowndown::Error>) {
    match result {
        Ok(content) => {
            println!("Conversion succeeded without token");
            validate_and_log_result(Ok(content.as_str()), "Without token");
        }
        Err(ref e) => {
            validate_and_log_result(Err(e), "Without token (expected)");
        }
    }
}

/// Create MarkdownDown instance with optional token.
fn create_markdown_instance(token: Option<&str>) -> MarkdownDown {
    match token {
        Some(t) => create_github_markdown_instance(t),
        None => MarkdownDown::new(),
    }
}

/// Test authentication with optional token.
async fn test_with_optional_token(
    test_url: &str,
    config: &IntegrationTestConfig,
    token: Option<&str>,
) -> Result<Markdown, markdowndown::Error> {
    let description = if token.is_some() { "with" } else { "without" };
    println!("Testing {description} GitHub token");
    
    let md = create_markdown_instance(token);
    convert_with_rate_limit(&md, test_url, config).await
}

/// Test authentication with and without token.
async fn test_authentication_with_and_without_token(
    test_url: &str,
    config: &IntegrationTestConfig,
    token: &str,
) {
    let result_no_token = test_with_optional_token(test_url, config, None).await;
    let result_with_token = test_with_optional_token(test_url, config, Some(token)).await;
    compare_auth_results(result_no_token, result_with_token);
}

/// Test authentication when no token is available.
async fn test_authentication_no_token_available(test_url: &str, config: &IntegrationTestConfig) {
    println!("No GitHub token available - testing without token only");
    let result_no_token = test_with_optional_token(test_url, config, None).await;
    handle_no_token_result(result_no_token);
}

/// Test GitHub authentication scenarios
#[tokio::test]
async fn test_github_authentication() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();
    let test_url = "https://github.com/rust-lang/rust/issues/1";

    if let Some(token) = &config.github_token {
        test_authentication_with_and_without_token(test_url, &config, token).await;
    } else {
        test_authentication_no_token_available(test_url, &config).await;
    }

    Ok(())
}

/// Check if content indicates an expected error response.
fn is_expected_error_content(content: &str) -> bool {
    content.contains("Error")
        || content.contains("not found")
        || content.contains("404")
        || content.len() < 100
}

/// Validate unexpected success result in error test case.
fn validate_unexpected_success(content: &str, description: &str) {
    println!("  Unexpected success: {} chars", content.len());
    assert!(
        is_expected_error_content(content),
        "Unexpected success content for {description}"
    );
}

/// Test a single error case scenario.
async fn test_error_case(
    md: &MarkdownDown,
    url: &str,
    description: &str,
    config: &IntegrationTestConfig,
) {
    println!("Testing error case: {description}");
    let result = convert_with_rate_limit(md, url, config).await;

    match result {
        Ok(markdown) => {
            validate_unexpected_success(markdown.as_str(), description);
        }
        Err(ref error) => {
            validate_and_log_result(Err(error), &format!("Error case: {description}"));
        }
    }
}

/// Test GitHub error scenarios
#[tokio::test]
async fn test_github_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    let md = if let Some(token) = &config.github_token {
        create_github_markdown_instance(token)
    } else {
        MarkdownDown::new()
    };

    let error_cases = [
        (
            "https://github.com/nonexistent/repo/issues/1".to_string(),
            "Non-existent repository",
        ),
        (
            format!("https://github.com/rust-lang/rust/issues/{}", NONEXISTENT_ISSUE_NUMBER),
            "Non-existent issue",
        ),
        (
            format!("https://github.com/rust-lang/rust/pull/{}", NONEXISTENT_PR_NUMBER),
            "Non-existent pull request",
        ),
    ];

    for (url, description) in error_cases.iter() {
        test_error_case(&md, url.as_str(), description, &config).await;
    }

    Ok(())
}

/// Measure and handle conversion operations with performance tracking.
/// This is a wrapper around handle_conversion_with_timing with performance-specific logging.
async fn measure_and_handle<F, Fut>(
    operation: F,
    config: &IntegrationTestConfig,
    context: &str,
) -> Result<ConversionResult, Box<dyn std::error::Error>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Markdown, markdowndown::Error>>,
{
    println!("Performance testing: {context}");
    
    let start = Instant::now();
    let result = operation().await;
    let duration = start.elapsed();
    
    match result {
        Ok(markdown) => {
            let content = markdown.as_str();
            let content_length = content.len();
            println!("  Duration: {duration:?}, Content: {content_length} chars");
            
            validate_markdown_content(content, context)?;
            
            if duration >= config.default_timeout() {
                return Err(format!("Conversion took too long for {context}: {duration:?}").into());
            }
            
            Ok(ConversionResult {
                duration,
                content_length,
                success: true,
            })
        }
        Err(e) if is_acceptable_github_error_type(&e) => {
            println!("  Failed: {e} (may be acceptable)");
            Ok(ConversionResult {
                duration,
                content_length: 0,
                success: false,
            })
        }
        Err(e) => Err(e.into()),
    }
}

/// Print performance summary report from collected metrics.
fn print_performance_summary(total_duration: std::time::Duration, total_chars: usize, successes: usize) {
    if successes > 0 {
        println!("GitHub Performance Summary:");
        println!("  Total successful requests: {successes}");
        println!("  Total time: {total_duration:?}");
        println!("  Total content: {total_chars} chars");
        println!(
            "  Average time per request: {:?}",
            total_duration / successes as u32
        );
        println!("  Average chars per request: {}", total_chars / successes);
    } else {
        println!("No successful requests - may be due to rate limiting or permissions");
    }
}

/// Summary of performance metrics collected across multiple conversions.
struct PerformanceSummary {
    total_duration: std::time::Duration,
    total_chars: usize,
    successes: usize,
}

/// Collect performance metrics for a list of URLs.
async fn collect_performance_metrics(
    md: &MarkdownDown,
    test_urls: &[&str],
    config: &IntegrationTestConfig,
) -> Result<PerformanceSummary, Box<dyn std::error::Error>> {
    let mut total_duration = std::time::Duration::from_secs(0);
    let mut total_chars = 0;
    let mut successes = 0;

    for url in test_urls.iter() {
        let url_owned = url.to_string();
        let result = measure_and_handle(
            || convert_with_rate_limit(md, &url_owned, config),
            config,
            &url_owned,
        )
        .await?;
        
        if result.success {
            total_duration += result.duration;
            total_chars += result.content_length;
            successes += 1;
        }
    }

    Ok(PerformanceSummary {
        total_duration,
        total_chars,
        successes,
    })
}

/// Performance test for GitHub conversion
#[tokio::test]
async fn test_github_performance() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = match GithubTestContext::setup() {
        Ok(ctx) => ctx,
        Err(msg) => {
            println!("Skipping GitHub performance test - {msg}");
            return Ok(());
        }
    };

    if ctx.config.skip_slow_tests {
        println!("Skipping GitHub performance test - slow tests skipped");
        return Ok(());
    }

    let test_urls = [
        "https://github.com/rust-lang/rust/issues/1",
        &format!("https://github.com/tokio-rs/tokio/issues/{}", TEST_TOKIO_ISSUE_NUMBER),
    ];

    let summary = collect_performance_metrics(&ctx.md, &test_urls, &ctx.config).await?;
    print_performance_summary(summary.total_duration, summary.total_chars, summary.successes);

    Ok(())
}
