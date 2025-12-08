//! Integration tests for GitHub issues and pull requests conversion
//!
//! Tests the library's ability to convert GitHub issues and PRs to markdown.

use markdowndown::MarkdownDown;
use std::time::Instant;

use super::{IntegrationTestConfig, TestUrls, TestUtils};

/// Test conversion of GitHub issues and pull requests
#[tokio::test]
async fn test_github_conversions() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_github() {
        println!("Skipping GitHub tests - no token available or external services disabled");
        return Ok(());
    }

    let github_config = markdowndown::Config::builder()
        .github_token(config.github_token.as_ref().unwrap())
        .build();
    let md = MarkdownDown::with_config(github_config);

    for (url, description) in TestUrls::GITHUB_TEST_URLS.iter() {
        println!("Testing: {description} - {url}");

        // Apply rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let start = Instant::now();
        let result = md.convert_url(url).await;
        let duration = start.elapsed();

        match result {
            Ok(markdown) => {
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

                // GitHub-specific content checks
                assert!(content.contains('#'), "Should have headers");
                assert!(
                    frontmatter.contains("github.com"),
                    "Should reference GitHub in frontmatter"
                );

                // Performance check
                assert!(
                    duration < config.default_timeout(),
                    "Conversion took too long for {description}: {duration:?}"
                );

                println!(
                    "✓ {description} converted successfully ({} chars, {duration:?})",
                    content.len()
                );
            }
            Err(e) => {
                println!("⚠ {description} failed: {e}");
                // For some URLs, failure may be expected (rate limiting, permissions, etc.)
                assert!(!e.to_string().is_empty(), "Error should have a message");

                // Check if it's a recoverable error that might indicate rate limiting
                if e.to_string().contains("rate limit") || e.to_string().contains("403") {
                    println!("  Rate limit or permission error - this is acceptable");
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}

/// Test specific GitHub issue conversion
#[tokio::test]
async fn test_github_issue_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_github() {
        println!("Skipping GitHub issue test - no token available or external services disabled");
        return Ok(());
    }

    let github_config = markdowndown::Config::builder()
        .github_token(config.github_token.as_ref().unwrap())
        .build();
    let md = MarkdownDown::with_config(github_config);

    // Test the historic first issue in Rust repository
    let url = "https://github.com/rust-lang/rust/issues/1";

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let result = md.convert_url(url).await;

    match result {
        Ok(markdown) => {
            let content = markdown.as_str();

            // Validate GitHub issue specific content
            assert!(content.len() > 100, "Issue should have substantial content");
            assert!(content.contains('#'), "Should have headers");

            // Check for typical issue elements
            let frontmatter = markdown.frontmatter().unwrap();
            assert!(frontmatter.contains("github.com"));
            assert!(frontmatter.contains("rust-lang/rust"));
            assert!(frontmatter.contains("issues/1") || frontmatter.contains("issue_number"));

            println!(
                "✓ GitHub issue #1 converted successfully ({} chars)",
                content.len()
            );
        }
        Err(e) => {
            println!("⚠ GitHub issue #1 conversion failed: {e}");
            // May fail due to rate limiting or permissions
            if e.to_string().contains("rate limit")
                || e.to_string().contains("403")
                || e.to_string().contains("401")
            {
                println!("  Authentication or rate limit issue - acceptable in testing");
                return Ok(());
            } else {
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test GitHub pull request conversion
#[tokio::test]
async fn test_github_pull_request_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_github() {
        println!("Skipping GitHub PR test - no token available or external services disabled");
        return Ok(());
    }

    let github_config = markdowndown::Config::builder()
        .github_token(config.github_token.as_ref().unwrap())
        .build();
    let md = MarkdownDown::with_config(github_config);

    // Test a pull request
    let url = "https://github.com/serde-rs/serde/pull/2000";

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let result = md.convert_url(url).await;

    match result {
        Ok(markdown) => {
            let content = markdown.as_str();

            // Validate PR specific content
            assert!(content.len() > 50, "PR should have meaningful content");

            // Check frontmatter
            let frontmatter = markdown.frontmatter().unwrap();
            assert!(frontmatter.contains("github.com"));
            assert!(frontmatter.contains("serde-rs/serde"));
            assert!(frontmatter.contains("pull/2000") || frontmatter.contains("pr_number"));

            println!(
                "✓ GitHub PR #2000 converted successfully ({} chars)",
                content.len()
            );
        }
        Err(e) => {
            println!("⚠ GitHub PR conversion failed: {e}");
            // May fail due to rate limiting, permissions, or non-existent PR
            if e.to_string().contains("rate limit")
                || e.to_string().contains("403")
                || e.to_string().contains("401")
                || e.to_string().contains("404")
            {
                println!("  Authentication, rate limit, or not found - acceptable in testing");
                return Ok(());
            } else {
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test GitHub URL format detection
#[tokio::test]
async fn test_github_url_detection() -> Result<(), Box<dyn std::error::Error>> {
    let _config = IntegrationTestConfig::from_env();

    // Test URL detection (doesn't require token)
    let github_urls = [
        "https://github.com/rust-lang/rust/issues/12345",
        "https://github.com/microsoft/vscode/pull/67890",
        "https://github.com/facebook/react/issues/1",
        "https://api.github.com/repos/owner/repo/issues/123", // API URL format
    ];

    for url in github_urls.iter() {
        println!("Testing URL detection: {url}");

        let detected_type = markdowndown::detect_url_type(url)?;
        assert_eq!(
            detected_type,
            markdowndown::types::UrlType::GitHubIssue,
            "Should detect as GitHub issue/PR: {url}"
        );
    }

    println!("✓ All GitHub URL formats detected correctly");
    Ok(())
}

/// Test GitHub rate limiting behavior
#[tokio::test]
async fn test_github_rate_limiting() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_github() || config.skip_slow_tests {
        println!("Skipping GitHub rate limiting test - no token available, external services disabled, or slow tests skipped");
        return Ok(());
    }

    let github_config = markdowndown::Config::builder()
        .github_token(config.github_token.as_ref().unwrap())
        .build();
    let md = MarkdownDown::with_config(github_config);

    // Test multiple rapid requests to trigger rate limiting behavior
    let urls = [
        "https://github.com/rust-lang/rust/issues/1",
        "https://github.com/rust-lang/rust/issues/2",
        "https://github.com/rust-lang/rust/issues/3",
    ];

    let mut successes = 0;
    let mut rate_limited = 0;
    let start = Instant::now();

    for (i, url) in urls.iter().enumerate() {
        println!("Request {}: {url}", i + 1);

        // Small delay to avoid overwhelming
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        let result = md.convert_url(url).await;

        match result {
            Ok(markdown) => {
                successes += 1;
                println!("  ✓ Success ({} chars)", markdown.as_str().len());
                assert!(TestUtils::validate_markdown_quality(markdown.as_str()));
            }
            Err(e) => {
                println!("  ⚠ Failed: {e}");
                if e.to_string().contains("rate limit") || e.to_string().contains("403") {
                    rate_limited += 1;
                    println!("    Rate limited - this is expected behavior");
                } else {
                    println!("    Unexpected error: {e}");
                }
            }
        }
    }

    let duration = start.elapsed();

    println!("Rate limiting test results:");
    println!("  Total requests: {}", urls.len());
    println!("  Successes: {successes}");
    println!("  Rate limited: {rate_limited}");
    println!("  Duration: {duration:?}");

    // Should handle rate limiting gracefully
    assert!(
        successes + rate_limited == urls.len(),
        "All requests should either succeed or be rate limited"
    );

    Ok(())
}

/// Test GitHub authentication scenarios
#[tokio::test]
async fn test_github_authentication() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    let test_url = "https://github.com/rust-lang/rust/issues/1";

    // Test without token
    println!("Testing without GitHub token");
    let md_no_token = MarkdownDown::new();
    TestUtils::apply_rate_limit(&config).await;
    let result_no_token = md_no_token.convert_url(test_url).await;

    // Test with token (if available)
    if let Some(token) = &config.github_token {
        println!("Testing with GitHub token");
        let github_config = markdowndown::Config::builder().github_token(token).build();
        let md_with_token = MarkdownDown::with_config(github_config);

        TestUtils::apply_rate_limit(&config).await;
        let result_with_token = md_with_token.convert_url(test_url).await;

        // Compare results
        match (result_no_token, result_with_token) {
            (Ok(content1), Ok(content2)) => {
                println!("Both conversions succeeded");
                assert!(TestUtils::validate_markdown_quality(content1.as_str()));
                assert!(TestUtils::validate_markdown_quality(content2.as_str()));
                // With token might have more detailed information
                println!("  No token: {} chars", content1.as_str().len());
                println!("  With token: {} chars", content2.as_str().len());
            }
            (Err(e1), Err(e2)) => {
                println!("Both conversions failed");
                println!("  No token error: {e1}");
                println!("  With token error: {e2}");
                // Both should fail gracefully
                assert!(!e1.to_string().is_empty());
                assert!(!e2.to_string().is_empty());
            }
            (Ok(content), Err(e)) => {
                println!("No-token succeeded, with-token failed: {e}");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
            }
            (Err(e), Ok(content)) => {
                println!("No-token failed, with-token succeeded: {e}");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
                // This is the expected case - token should provide better access
            }
        }
    } else {
        println!("No GitHub token available - testing without token only");
        match result_no_token {
            Ok(content) => {
                println!("Conversion succeeded without token");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
            }
            Err(e) => {
                println!("Conversion failed without token (expected): {e}");
                assert!(!e.to_string().is_empty());
                // Should fail gracefully with descriptive error
                assert!(
                    e.to_string().contains("auth")
                        || e.to_string().contains("token")
                        || e.to_string().contains("403")
                        || e.to_string().contains("401"),
                    "Error should indicate authentication issue"
                );
            }
        }
    }

    Ok(())
}

/// Test GitHub error scenarios
#[tokio::test]
async fn test_github_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    let md = if let Some(token) = &config.github_token {
        let github_config = markdowndown::Config::builder().github_token(token).build();
        MarkdownDown::with_config(github_config)
    } else {
        MarkdownDown::new()
    };

    let error_cases = [
        (
            "https://github.com/nonexistent/repo/issues/1",
            "Non-existent repository",
        ),
        (
            "https://github.com/rust-lang/rust/issues/999999",
            "Non-existent issue",
        ),
        (
            "https://github.com/rust-lang/rust/pull/999999",
            "Non-existent pull request",
        ),
    ];

    for (url, description) in error_cases.iter() {
        println!("Testing error case: {description}");

        // Rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(url).await;

        // Should fail gracefully
        match result {
            Ok(markdown) => {
                println!("  Unexpected success: {} chars", markdown.as_str().len());
                // If it succeeds, content should indicate the issue
                let content = markdown.as_str();
                assert!(
                    content.contains("Error")
                        || content.contains("not found")
                        || content.contains("404")
                        || content.len() < 100,
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

                // Should indicate the specific problem
                assert!(
                    error.to_string().contains("404")
                        || error.to_string().contains("not found")
                        || error.to_string().contains("nonexistent")
                        || error.to_string().contains("403")
                        || error.to_string().contains("401"),
                    "Error should indicate specific issue type"
                );
            }
        }
    }

    Ok(())
}

/// Performance test for GitHub conversion
#[tokio::test]
async fn test_github_performance() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_github() || config.skip_slow_tests {
        println!("Skipping GitHub performance test - no token available, external services disabled, or slow tests skipped");
        return Ok(());
    }

    let github_config = markdowndown::Config::builder()
        .github_token(config.github_token.as_ref().unwrap())
        .build();
    let md = MarkdownDown::with_config(github_config);

    let test_urls = [
        "https://github.com/rust-lang/rust/issues/1",
        "https://github.com/tokio-rs/tokio/issues/1000",
    ];

    let mut total_duration = std::time::Duration::from_secs(0);
    let mut total_chars = 0;
    let mut successes = 0;

    for url in test_urls.iter() {
        println!("Performance testing: {url}");

        // Rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let start = Instant::now();
        let result = md.convert_url(url).await;
        let duration = start.elapsed();

        match result {
            Ok(markdown) => {
                let content_length = markdown.as_str().len();
                total_duration += duration;
                total_chars += content_length;
                successes += 1;

                println!("  Duration: {duration:?}, Content: {content_length} chars");

                // Performance assertions
                assert!(
                    duration < config.default_timeout(),
                    "GitHub conversion took too long: {duration:?}"
                );

                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str()),
                    "Performance test should produce quality output"
                );
            }
            Err(e) => {
                println!("  Failed: {e} (may be acceptable)");
                // Rate limiting or permissions errors are acceptable
                if !e.to_string().contains("rate limit")
                    && !e.to_string().contains("403")
                    && !e.to_string().contains("401")
                {
                    return Err(e.into());
                }
            }
        }
    }

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

    Ok(())
}
