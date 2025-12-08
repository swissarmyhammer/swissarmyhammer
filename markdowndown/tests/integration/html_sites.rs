//! Integration tests for HTML website conversion
//!
//! Tests the library's ability to convert real HTML websites to markdown.

use markdowndown::MarkdownDown;
use std::time::Instant;

use super::{IntegrationTestConfig, TestUrls, TestUtils};

/// Test conversion of various HTML websites
#[tokio::test]
async fn test_html_site_conversions() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping HTML tests - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    for (url, description) in TestUrls::HTML_TEST_URLS.iter() {
        println!("Testing: {description} - {url}");

        // Apply rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let start = Instant::now();
        let result = md.convert_url(url).await;
        let duration = start.elapsed();

        // Should succeed for all test URLs
        assert!(
            result.is_ok(),
            "Failed to convert {description}: {:?}",
            result.err()
        );

        let markdown = result.unwrap();
        let content = markdown.as_str();

        // Basic quality checks
        assert!(
            TestUtils::validate_markdown_quality(content),
            "Poor quality markdown for {description}: content too short or invalid"
        );

        // Should have frontmatter
        assert!(
            markdown.frontmatter().is_some(),
            "Missing frontmatter for {description}"
        );

        let frontmatter = markdown.frontmatter().unwrap();
        assert!(
            TestUtils::validate_frontmatter(&frontmatter),
            "Invalid frontmatter for {description}"
        );

        // Performance check - should complete within reasonable time
        assert!(
            duration < config.default_timeout(),
            "Conversion took too long for {description}: {duration:?}"
        );

        println!(
            "✓ {description} converted successfully ({} chars, {duration:?})",
            content.len()
        );
    }

    Ok(())
}

/// Test Wikipedia page conversion specifically
#[tokio::test]
async fn test_wikipedia_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping Wikipedia test - external services disabled");
        return Ok(());
    }

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let md = MarkdownDown::new();
    let url = "https://en.wikipedia.org/wiki/Rust_(programming_language)";

    let result = md.convert_url(url).await?;

    // Validate Wikipedia-specific content
    let content = result.as_str();
    assert!(content.contains("Rust") || content.contains("programming language"));
    assert!(
        content.len() > 5000,
        "Wikipedia content should be substantial"
    );

    // Check for proper markdown structure
    assert!(content.contains('#'), "Should have headers");

    // Validate frontmatter
    let frontmatter = result.frontmatter().unwrap();
    assert!(frontmatter.contains("source_url"));
    assert!(frontmatter.contains("html2markdown") || frontmatter.contains("conversion_type"));

    println!(
        "✓ Wikipedia conversion successful ({} chars)",
        content.len()
    );
    Ok(())
}

/// Test Rust documentation conversion
#[tokio::test]
async fn test_rust_docs_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping Rust docs test - external services disabled");
        return Ok(());
    }

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let md = MarkdownDown::new();
    let url = "https://doc.rust-lang.org/book/ch01-00-getting-started.html";

    let result = md.convert_url(url).await?;

    // Validate Rust docs specific content
    let content = result.as_str();
    assert!(
        content.to_lowercase().contains("rust")
            || content.to_lowercase().contains("getting started")
    );
    assert!(
        content.len() > 500,
        "Documentation should have substantial content (got {} chars)",
        content.len()
    );

    // Check for typical documentation content patterns
    // Note: Some docs sites use JavaScript to load content, so we check for basic structure
    assert!(
        content.contains("```")
            || content.contains("    ")
            || content.contains("Rust")
            || content.contains("#"),
        "Should contain documentation structure or Rust-related content"
    );

    println!(
        "✓ Rust documentation conversion successful ({} chars)",
        content.len()
    );
    Ok(())
}

/// Test GitHub README conversion
#[tokio::test]
async fn test_github_readme_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping GitHub README test - external services disabled");
        return Ok(());
    }

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let md = MarkdownDown::new();
    let url = "https://github.com/rust-lang/rust/blob/master/README.md";

    let result = md.convert_url(url).await?;

    // Validate GitHub README content
    let content = result.as_str();
    assert!(content.to_lowercase().contains("rust"));
    assert!(content.len() > 500, "README should have meaningful content");

    // Should contain typical README elements
    assert!(content.contains('#'), "Should have headers");

    println!(
        "✓ GitHub README conversion successful ({} chars)",
        content.len()
    );
    Ok(())
}

/// Test httpbin HTML for controlled testing
#[tokio::test]
async fn test_simple_html_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping simple HTML test - external services disabled");
        return Ok(());
    }

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let md = MarkdownDown::new();
    let url = "https://httpbin.org/html";

    let result = md.convert_url(url).await?;

    // Validate basic HTML conversion
    let content = result.as_str();
    assert!(content.len() > 100, "Should have meaningful content");
    assert!(
        TestUtils::validate_markdown_quality(content),
        "Should be quality markdown"
    );

    // Check frontmatter
    let frontmatter = result.frontmatter().unwrap();
    assert!(
        frontmatter.contains("httpbin.org"),
        "Should reference source URL"
    );

    println!(
        "✓ Simple HTML conversion successful ({} chars)",
        content.len()
    );
    Ok(())
}

/// Performance benchmark for HTML conversion
#[tokio::test]
async fn test_html_conversion_performance() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() || config.skip_slow_tests {
        println!(
            "Skipping HTML performance test - external services disabled or slow tests skipped"
        );
        return Ok(());
    }

    let md = MarkdownDown::new();
    let mut total_duration = std::time::Duration::from_secs(0);
    let mut total_chars = 0;

    for (url, description) in TestUrls::HTML_TEST_URLS.iter().take(3) {
        println!("Benchmarking: {description}");

        // Rate limiting
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

        // Performance assertions
        assert!(
            duration < config.default_timeout(),
            "Conversion took too long: {duration:?}"
        );
    }

    println!("Performance Summary:");
    println!("  Total time: {total_duration:?}");
    println!("  Total content: {total_chars} chars");
    println!("  Average time per request: {:?}", total_duration / 3);
    println!("  Average chars per request: {}", total_chars / 3);

    Ok(())
}

/// Test error handling with invalid HTML URLs
#[tokio::test]
async fn test_html_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() {
        println!("Skipping HTML error tests - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    let error_cases = [
        ("https://httpbin.org/status/404", "HTTP 404 error"),
        ("https://httpbin.org/status/500", "HTTP 500 error"),
        (
            "https://invalid-domain-12345.com/page",
            "DNS resolution failure",
        ),
    ];

    for (url, description) in error_cases.iter() {
        println!("Testing error case: {description}");

        // Rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(url).await;

        // Should either succeed with fallback or fail gracefully
        match result {
            Ok(markdown) => {
                println!(
                    "  Succeeded with fallback: {} chars",
                    markdown.as_str().len()
                );
                // If it succeeded, validate it's reasonable content
                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str())
                        || markdown.as_str().contains("Error")
                        || markdown.as_str().contains("404")
                        || markdown.as_str().contains("500"),
                    "Unexpected success content for {description}"
                );
            }
            Err(error) => {
                println!("  Failed as expected: {error}");
                // Error should be descriptive
                assert!(
                    !error.to_string().is_empty(),
                    "Error message should not be empty"
                );
                assert!(
                    error.to_string().len() > 10,
                    "Error message should be descriptive"
                );
            }
        }
    }

    Ok(())
}

/// Test concurrent HTML conversions
#[tokio::test]
async fn test_concurrent_html_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_html() || config.skip_slow_tests {
        println!(
            "Skipping concurrent HTML test - external services disabled or slow tests skipped"
        );
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Test concurrent requests (but respect rate limiting)
    let urls = [
        "https://httpbin.org/html",
        "https://doc.rust-lang.org/book/ch01-00-getting-started.html",
    ];

    let start = Instant::now();
    let futures = urls.iter().map(|url| async {
        // Small delay to avoid overwhelming the service
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
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
        urls.len()
    );

    Ok(())
}
