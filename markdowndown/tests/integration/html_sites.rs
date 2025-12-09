//! Integration tests for HTML website conversion
//!
//! Tests the library's ability to convert real HTML websites to markdown.

use markdowndown::{types::Markdown, MarkdownDown};
use std::time::{Duration, Instant};

use super::{IntegrationTestConfig, TestUrlType, TestUrls, TestUtils};

/// HTTP status code strings used in error validation
const HTTP_NOT_FOUND_STR: &str = "404";
const HTTP_INTERNAL_ERROR_STR: &str = "500";

/// Check if HTML test should be skipped based on configuration
///
/// Returns true if the test should be skipped
fn should_skip_html_test(config: &IntegrationTestConfig, test_type: &str) -> bool {
    let skip = !config.can_test_html() || config.skip_slow_tests;
    if skip {
        println!("Skipping {test_type} - external services disabled or slow tests skipped");
    }
    skip
}

/// Setup test with rate limiting and return MarkdownDown instance
///
/// Returns None if the test should be skipped based on config
async fn setup_test_with_rate_limit(
    config: &IntegrationTestConfig,
    test_name: &str,
) -> Option<MarkdownDown> {
    if !config.can_test_html() {
        println!("Skipping {test_name} - external services disabled");
        return None;
    }

    TestUtils::apply_rate_limit(config).await;
    Some(MarkdownDown::new())
}

/// Validate converted markdown content has substance and structure
fn validate_converted_content(
    content: &str,
    expected_patterns: &[&str],
    description: &str,
) -> Result<(), String> {
    // Use semantic validation - check that content has markdown structure
    if !TestUtils::validate_markdown_quality(content) {
        return Err(format!(
            "{description} should have valid markdown structure (got {} chars, {} lines)",
            content.len(),
            content.lines().count()
        ));
    }

    for pattern in expected_patterns {
        if !content.contains(pattern) {
            return Err(format!(
                "{description} should contain '{pattern}' pattern"
            ));
        }
    }

    Ok(())
}

/// Print standardized conversion success message
fn print_conversion_success(description: &str, content_len: usize, duration: Option<Duration>) {
    if let Some(dur) = duration {
        println!("✓ {description} converted successfully ({content_len} chars, {dur:?})");
    } else {
        println!("✓ {description} converted successfully ({content_len} chars)");
    }
}

/// Validate that frontmatter contains required keys
fn validate_frontmatter_contains(
    frontmatter: &str,
    required_keys: &[&str],
    description: &str,
) -> Result<(), String> {
    for key in required_keys {
        if !frontmatter.contains(key) {
            return Err(format!("{description} frontmatter should contain '{key}'"));
        }
    }
    Ok(())
}

/// Helper function to perform a full HTML conversion test with validation
///
/// This helper function handles the common pattern of:
/// 1. Setting up test environment with rate limiting
/// 2. Executing the conversion
/// 3. Validating the result
async fn test_html_conversion(
    test_name: &str,
    url: &str,
    description: &str,
    expected_patterns: &[&str],
) -> Result<Markdown, Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();
    let md = match setup_test_with_rate_limit(&config, test_name).await {
        Some(md) => md,
        None => return Ok(Markdown::default()),
    };

    let timeout = config.default_timeout();
    let start = Instant::now();
    let result = md.convert_url(url).await?;
    let duration = start.elapsed();

    let content = result.as_str();

    // Validate content has proper markdown structure
    validate_converted_content(content, expected_patterns, description)
        .map_err(|e| e.to_string())?;

    // Validate frontmatter exists and is valid
    let frontmatter = result
        .frontmatter()
        .ok_or_else(|| format!("Missing frontmatter for {description}"))?;

    if !TestUtils::validate_frontmatter(&frontmatter) {
        return Err(format!("Invalid frontmatter for {description}").into());
    }

    // Performance check
    if duration >= timeout {
        return Err(format!(
            "Conversion took too long for {description}: {duration:?}"
        )
        .into());
    }

    print_conversion_success(description, content.len(), Some(duration));

    Ok(result)
}

/// Helper function to test a specific site conversion by URL type
async fn test_specific_site_conversion(
    url_type: TestUrlType,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (url, description) = match TestUrls::get_url_by_type(url_type) {
        Some(url_info) => url_info,
        None => {
            println!("Skipping {test_name} - no URL configured for type {url_type:?}");
            return Ok(());
        }
    };

    test_html_conversion(test_name, url, description, &[]).await?;
    Ok(())
}

/// Test conversion of various HTML websites
#[tokio::test]
async fn test_html_site_conversions() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();
    if !config.can_test_html() {
        println!("Skipping HTML tests - external services disabled");
        return Ok(());
    }

    for (url, description, _) in TestUrls::HTML_TEST_URLS.iter() {
        println!("Testing: {description} - {url}");

        TestUtils::apply_rate_limit(&config).await;

        test_html_conversion(
            description,
            url,
            description,
            &[], // No specific patterns required - validate structure instead
        )
        .await?;
    }

    Ok(())
}

/// Test Wikipedia page conversion specifically
#[tokio::test]
async fn test_wikipedia_conversion() -> Result<(), Box<dyn std::error::Error>> {
    test_specific_site_conversion(TestUrlType::Complex, "Wikipedia test").await
}

/// Test Rust documentation conversion
#[tokio::test]
async fn test_rust_docs_conversion() -> Result<(), Box<dyn std::error::Error>> {
    test_specific_site_conversion(TestUrlType::Documentation, "Rust docs test").await
}

/// Test GitHub README conversion
#[tokio::test]
async fn test_github_readme_conversion() -> Result<(), Box<dyn std::error::Error>> {
    test_specific_site_conversion(TestUrlType::SourceCode, "GitHub README test").await
}

/// Test httpbin HTML for controlled testing
#[tokio::test]
async fn test_simple_html_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let (url, description) = match TestUrls::get_url_by_type(TestUrlType::Simple) {
        Some(url_info) => url_info,
        None => {
            println!("Skipping simple HTML test - no simple URL configured");
            return Ok(());
        }
    };

    let result = test_html_conversion(
        "simple HTML test",
        url,
        description,
        &[],
    )
    .await?;

    // Verify frontmatter contains the source domain
    let frontmatter = result.frontmatter().unwrap();
    
    // Use proper URL parsing to extract domain reliably
    let url_domain = url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| {
            // Fallback for tests, but should not occur with valid URLs
            println!("Warning: Could not parse URL domain from {url}");
            String::new()
        });
    
    if !url_domain.is_empty() {
        validate_frontmatter_contains(&frontmatter, &[&url_domain], description)?;
    }

    Ok(())
}

/// Performance benchmark for HTML conversion
#[tokio::test]
async fn test_html_conversion_performance() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if should_skip_html_test(&config, "HTML performance test") {
        return Ok(());
    }

    let md = MarkdownDown::new();
    let mut total_duration = Duration::from_secs(0);
    let mut total_chars = 0;

    // Benchmark all configured URLs to get a representative sample
    let url_count = TestUrls::HTML_TEST_URLS.len();
    
    for (url, description, _) in TestUrls::HTML_TEST_URLS.iter() {
        println!("Benchmarking: {description}");

        TestUtils::apply_rate_limit(&config).await;

        let start = Instant::now();
        let result = md.convert_url(url).await?;
        let duration = start.elapsed();

        total_duration += duration;
        total_chars += result.as_str().len();

        println!(
            "  Duration: {duration:?}, Content: {} chars",
            result.as_str().len()
        );

        assert!(
            duration < config.default_timeout(),
            "Conversion took too long: {duration:?}"
        );
    }

    println!("Performance Summary:");
    println!("  Total time: {total_duration:?}");
    println!("  Total content: {total_chars} chars");
    println!("  Average time per request: {:?}", total_duration / url_count as u32);
    println!("  Average chars per request: {}", total_chars / url_count);

    Ok(())
}

/// Test a single error case and validate the result
async fn test_error_case(
    md: &MarkdownDown,
    url: &str,
    description: &str,
    config: &IntegrationTestConfig,
) {
    println!("Testing error case: {description}");

    TestUtils::apply_rate_limit(config).await;

    let result = md.convert_url(url).await;

    validate_error_or_fallback(result, description);
}

/// Check if content appears to be an error page
fn is_error_page_content(content: &str) -> bool {
    let content_lower = content.to_lowercase();
    content_lower.contains("error")
        || content_lower.contains(HTTP_NOT_FOUND_STR)
        || content_lower.contains(HTTP_INTERNAL_ERROR_STR)
        || content_lower.contains("not found")
        || content_lower.contains("server error")
}

/// Validate fallback content that succeeded but may be an error page
fn validate_fallback_content(content: &str) {
    // Either it's valid markdown or it's an error page with error indicators
    assert!(
        TestUtils::validate_markdown_quality(content) || is_error_page_content(content),
        "Unexpected success content"
    );
}

/// Check if error string contains HTTP-related keywords
fn validate_http_error(error_string: &str) -> bool {
    let lower = error_string.to_lowercase();
    lower.contains("http") || lower.contains("request") || lower.contains("status")
}

/// Helper to validate an error message contains meaningful keywords
fn validate_error_string(error_string: &str, expected_keywords: &[&str], context: &str) {
    assert!(!error_string.is_empty(), "{} error should have a message", context);
    
    let lower = error_string.to_lowercase();
    let has_keyword = expected_keywords.iter().any(|kw| lower.contains(kw));
    assert!(
        has_keyword,
        "{} error should mention one of {:?}: {}",
        context, expected_keywords, error_string
    );
}

/// Validate error message contains meaningful information
fn validate_error_message(error: &markdowndown::Error) {
    let error_string = error.to_string();
    
    // Verify the error is one of the expected variants
    match error {
        markdowndown::Error::Http(_) => {
            assert!(
                validate_http_error(&error_string),
                "HTTP error should mention http/request/status: {}",
                error_string
            );
        }
        markdowndown::Error::Io(_) => {
            assert!(!error_string.is_empty(), "IO error should have a message");
        }
        markdowndown::Error::Url(_) => {
            assert!(!error_string.is_empty(), "URL error should have a message");
        }
        markdowndown::Error::Other(msg) => {
            assert!(!msg.is_empty(), "Other error message should not be empty");
        }
    }
}

/// Validate a result that may be an error or a fallback success
fn validate_error_or_fallback(
    result: Result<Markdown, markdowndown::Error>,
    description: &str,
) {
    match result {
        Ok(markdown) => {
            println!("  Succeeded with fallback: {} chars", markdown.as_str().len());
            validate_fallback_content(markdown.as_str());
        }
        Err(error) => {
            println!("  Failed as expected: {error}");
            validate_error_message(&error);
        }
    }
}

/// Test error handling with invalid HTML URLs
#[tokio::test]
async fn test_html_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();
    let md = match setup_test_with_rate_limit(&config, "HTML error tests").await {
        Some(md) => md,
        None => return Ok(()),
    };

    // Use configured error test URLs from TestUrls
    for (url, description, _expected_indicator) in TestUrls::ERROR_TEST_URLS.iter() {
        test_error_case(&md, url, description, &config).await;
    }

    Ok(())
}

/// Test concurrent HTML conversions
#[tokio::test]
async fn test_concurrent_html_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if should_skip_html_test(&config, "concurrent HTML test") {
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Use configured number of test URLs for concurrent testing
    let test_urls: Vec<&str> = TestUrls::HTML_TEST_URLS
        .iter()
        .take(config.concurrent_test_count)
        .map(|(url, _, _)| *url)
        .collect();

    let start = Instant::now();
    let futures = test_urls.iter().map(|url| async {
        // Apply rate limiting to avoid overwhelming services
        tokio::time::sleep(config.request_delay()).await;
        md.convert_url(url).await
    });

    let results = futures::future::join_all(futures).await;
    let duration = start.elapsed();

    // All should succeed or fail gracefully
    let mut successes = 0;
    for result in results {
        match result {
            Ok(markdown) => {
                assert!(TestUtils::validate_markdown_quality(markdown.as_str()));
                successes += 1;
            }
            Err(e) => {
                println!("Concurrent request failed (acceptable): {e}");
            }
        }
    }

    assert!(
        successes > 0,
        "At least one concurrent request should succeed"
    );
    println!(
        "✓ Concurrent test completed: {successes}/{} succeeded in {duration:?}",
        test_urls.len()
    );

    Ok(())
}
