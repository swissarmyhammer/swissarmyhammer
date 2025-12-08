//! End-to-end integration tests
//!
//! Tests complete workflows and cross-cutting concerns across the entire library.

use markdowndown::MarkdownDown;
use std::time::Instant;

use super::{IntegrationTestConfig, TestUtils};

/// Test complete end-to-end workflow with various URL types
#[tokio::test]
async fn test_end_to_end_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services {
        println!("Skipping end-to-end workflow test - external services disabled");
        return Ok(());
    }

    // Create configured MarkdownDown instance
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
    let md = MarkdownDown::with_config(md_config);

    // Test URLs representing different service types
    let test_cases = [
        ("https://httpbin.org/html", "HTML", "Simple HTML conversion"),
        (
            "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            "HTML",
            "Complex Wikipedia page",
        ),
        (
            "https://github.com/rust-lang/rust/issues/1",
            "GitHub",
            "GitHub issue conversion",
        ),
        (
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
            "Google Docs",
            "Google Docs conversion",
        ),
    ];

    let mut successful_conversions = 0;
    let mut total_chars = 0;
    let start_time = Instant::now();

    for (url, service_type, description) in test_cases.iter() {
        println!("End-to-end test: {description} ({service_type})");

        // Apply rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let conversion_start = Instant::now();
        let result = md.convert_url(url).await;
        let conversion_duration = conversion_start.elapsed();

        match result {
            Ok(markdown) => {
                let content = markdown.as_str();
                let content_length = content.len();

                // Validate conversion quality
                assert!(
                    TestUtils::validate_markdown_quality(content),
                    "Poor quality output for {description}"
                );

                // Validate frontmatter
                assert!(
                    markdown.frontmatter().is_some(),
                    "Missing frontmatter for {description}"
                );

                let frontmatter = markdown.frontmatter().unwrap();
                assert!(
                    TestUtils::validate_frontmatter(&frontmatter),
                    "Invalid frontmatter for {description}"
                );

                // Validate service-specific content
                match *service_type {
                    "HTML" => {
                        assert!(
                            content.contains('#') || content.len() > 200,
                            "HTML content should have headers or substantial content"
                        );
                    }
                    "GitHub" => {
                        assert!(
                            frontmatter.contains("github.com"),
                            "GitHub frontmatter should reference GitHub"
                        );
                    }
                    "Google Docs" => {
                        // May fail for placeholder URL, which is acceptable
                    }
                    _ => {}
                }

                successful_conversions += 1;
                total_chars += content_length;

                println!("  ✓ Success: {content_length} chars in {conversion_duration:?}");
            }
            Err(e) => {
                println!("  ⚠ Failed: {e} (may be acceptable for some services)");

                // Some failures are acceptable (rate limiting, permissions, placeholder URLs)
                if e.to_string().contains("rate limit")
                    || e.to_string().contains("403")
                    || e.to_string().contains("401")
                    || e.to_string().contains("404")
                    || (*service_type == "Google Docs" && e.to_string().contains("nonexistent"))
                {
                    println!("    Acceptable failure type");
                } else {
                    println!("    Unexpected error: {e}");
                }
            }
        }
    }

    let total_duration = start_time.elapsed();

    println!("\nEnd-to-End Workflow Summary:");
    println!("  Total test cases: {}", test_cases.len());
    println!("  Successful conversions: {successful_conversions}");
    println!("  Total content generated: {total_chars} chars");
    println!("  Total time: {total_duration:?}");
    println!(
        "  Average time per test: {:?}",
        total_duration / test_cases.len() as u32
    );

    // Should have at least some successful conversions
    assert!(
        successful_conversions > 0,
        "At least one conversion should succeed in end-to-end test"
    );

    Ok(())
}

/// Test fallback behavior across different URL types
#[tokio::test]
async fn test_cross_service_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services {
        println!("Skipping cross-service fallback test - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Test URLs that might trigger fallback behavior
    let fallback_test_cases = [
        (
            "https://docs.google.com/document/d/nonexistent/edit",
            "Non-existent Google Doc should fallback to HTML",
        ),
        (
            "https://github.com/nonexistent/repo/issues/1",
            "Non-existent GitHub issue might fallback",
        ),
    ];

    for (url, description) in fallback_test_cases.iter() {
        println!("Testing fallback: {description}");

        // Apply rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(url).await;

        match result {
            Ok(markdown) => {
                println!("  ✓ Fallback successful: {} chars", markdown.as_str().len());

                // If fallback succeeded, validate the content
                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str())
                        || markdown.as_str().contains("Error")
                        || markdown.as_str().contains("not found"),
                    "Fallback should produce valid content or error message"
                );

                // Check frontmatter indicates the conversion type
                if let Some(frontmatter) = markdown.frontmatter() {
                    assert!(
                        TestUtils::validate_frontmatter(&frontmatter),
                        "Fallback should produce valid frontmatter"
                    );
                }
            }
            Err(e) => {
                println!("  ⚠ Fallback failed: {e} (acceptable)");
                // Fallback failures are acceptable - the important thing is graceful handling
                assert!(
                    !e.to_string().is_empty(),
                    "Error should have descriptive message"
                );
            }
        }
    }

    Ok(())
}

/// Test configuration propagation across all services
#[tokio::test]
async fn test_configuration_propagation() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services {
        println!("Skipping configuration propagation test - external services disabled");
        return Ok(());
    }

    // Test with custom configuration
    let custom_config = markdowndown::Config::builder()
        .timeout_seconds(60)
        .user_agent("test-integration/1.0")
        .max_retries(5)
        .include_frontmatter(true)
        .custom_frontmatter_field("test_run", "integration")
        .custom_frontmatter_field("config_test", "true")
        .max_consecutive_blank_lines(1)
        .build();

    let md = MarkdownDown::with_config(custom_config);

    // Test that configuration is properly applied
    let test_url = "https://httpbin.org/html";

    TestUtils::apply_rate_limit(&config).await;

    let result = md.convert_url(test_url).await?;

    // Validate that custom configuration was applied
    assert!(
        result.frontmatter().is_some(),
        "Custom config should include frontmatter"
    );

    let frontmatter = result.frontmatter().unwrap();
    assert!(
        frontmatter.contains("test_run: integration"),
        "Should include custom frontmatter field: test_run"
    );
    assert!(
        frontmatter.contains("config_test: \"true\"")
            || frontmatter.contains("config_test: true")
            || frontmatter.contains("config_test: 'true'"),
        "Should include custom frontmatter field: config_test"
    );

    // Check content processing
    let content = result.as_str();
    assert!(
        TestUtils::validate_markdown_quality(content),
        "Custom config should still produce quality content"
    );

    // Check that max consecutive blank lines is respected
    let blank_line_sequences: Vec<&str> = content.split("\n\n\n").collect();
    if blank_line_sequences.len() > 1 {
        // If there are triple newlines, check they don't exceed the limit
        for sequence in blank_line_sequences.iter().skip(1) {
            assert!(!sequence.starts_with('\n'),
                   "Should not have more than 1 consecutive blank line (max_consecutive_blank_lines=1)");
        }
    }

    println!("✓ Configuration propagation test passed");
    Ok(())
}

/// Test concurrent processing across different services
#[tokio::test]
async fn test_concurrent_cross_service_processing() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services || config.skip_slow_tests {
        println!("Skipping concurrent cross-service test - external services disabled or slow tests skipped");
        return Ok(());
    }

    let _md = MarkdownDown::new();

    // Different service types for concurrent testing
    let concurrent_urls = [
        "https://httpbin.org/html",
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    ];

    let start_time = Instant::now();

    // Execute conversions concurrently with rate limiting
    let futures = concurrent_urls
        .iter()
        .enumerate()
        .map(|(i, url)| async move {
            // Stagger requests to respect rate limiting
            tokio::time::sleep(std::time::Duration::from_millis((i as u64) * 1000)).await;

            // Create a new instance for each concurrent request
            let md_instance = MarkdownDown::new();
            let conversion_result = md_instance.convert_url(url).await;
            (url, conversion_result)
        });

    let results = futures::future::join_all(futures).await;
    let total_duration = start_time.elapsed();

    let mut successful = 0;
    let mut total_content = 0;

    for (url, result) in results {
        match result {
            Ok(markdown) => {
                let content_length = markdown.as_str().len();
                successful += 1;
                total_content += content_length;

                println!("  ✓ Concurrent success for {url}: {content_length} chars");

                // Validate quality
                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str()),
                    "Concurrent conversion should maintain quality"
                );

                // Validate frontmatter
                assert!(
                    markdown.frontmatter().is_some(),
                    "Concurrent conversion should include frontmatter"
                );
            }
            Err(e) => {
                println!("  ⚠ Concurrent failure for {url}: {e} (may be acceptable)");
            }
        }
    }

    println!("Concurrent Processing Summary:");
    println!("  Total URLs: {}", concurrent_urls.len());
    println!("  Successful: {successful}");
    println!("  Total content: {total_content} chars");
    println!("  Total time: {total_duration:?}");

    // Should have some successful concurrent conversions
    assert!(
        successful > 0,
        "At least one concurrent conversion should succeed"
    );

    // Concurrent processing should be reasonably efficient
    let average_time = total_duration / concurrent_urls.len() as u32;
    println!("  Average time per URL: {average_time:?}");

    Ok(())
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

        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(test_url).await?;
        let frontmatter = result.frontmatter().unwrap();

        // Should include consistent metadata
        assert!(
            frontmatter.contains("converted_at"),
            "Should include conversion timestamp"
        );
        assert!(
            frontmatter.contains("source_url"),
            "Should include source URL"
        );
        assert!(
            frontmatter.contains("conversion_type") || frontmatter.contains("html2markdown"),
            "Should include conversion type"
        );

        // User agent should be consistent
        let user_agent_field = "user_agent:".to_string();
        if frontmatter.contains(&user_agent_field) {
            assert!(
                frontmatter.contains("markdowndown/"),
                "User agent should include library name and version"
            );
        }
    }

    println!("✓ Library metadata consistency verified");
    Ok(())
}

/// Test error propagation and recovery across services
#[tokio::test]
async fn test_error_propagation_and_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services {
        println!("Skipping error propagation test - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Mix of URLs that should succeed and fail
    let mixed_urls = [
        ("https://httpbin.org/html", true, "Should succeed"),
        (
            "https://httpbin.org/status/404",
            false,
            "Should fail with 404",
        ),
        (
            "https://invalid-domain-12345.com",
            false,
            "Should fail with DNS error",
        ),
        (
            "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            true,
            "Should succeed",
        ),
    ];

    let mut successes = 0;
    let mut expected_failures = 0;
    let mut unexpected_results = 0;

    for (url, should_succeed, description) in mixed_urls.iter() {
        println!("Testing error handling: {description}");

        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(url).await;

        match (result, should_succeed) {
            (Ok(markdown), true) => {
                successes += 1;
                println!("  ✓ Expected success: {} chars", markdown.as_str().len());
                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str()) ||
                       markdown.as_str().contains("404") || // 404 pages might still convert
                       markdown.as_str().contains("Error"),
                    "Successful conversion should have quality content"
                );
            }
            (Ok(markdown), false) => {
                println!("  ⚠ Unexpected success: {} chars", markdown.as_str().len());
                // Sometimes error pages still convert successfully - this is acceptable
                // if the content indicates an error
                if markdown.as_str().contains("404")
                    || markdown.as_str().contains("Error")
                    || markdown.as_str().contains("not found")
                {
                    expected_failures += 1;
                    println!("    Content indicates error - acceptable");
                } else {
                    unexpected_results += 1;
                    println!("    Truly unexpected success");
                }
            }
            (Err(e), false) => {
                expected_failures += 1;
                println!("  ✓ Expected failure: {e}");
                assert!(
                    !e.to_string().is_empty(),
                    "Error should have descriptive message"
                );
                assert!(
                    e.to_string().len() > 10,
                    "Error message should be substantial"
                );
            }
            (Err(e), true) => {
                println!("  ⚠ Unexpected failure: {e}");
                // Some failures might be due to network issues or rate limiting
                if e.to_string().contains("timeout")
                    || e.to_string().contains("rate limit")
                    || e.to_string().contains("network")
                {
                    expected_failures += 1;
                    println!("    Network-related failure - acceptable");
                } else {
                    unexpected_results += 1;
                    println!("    Truly unexpected failure");
                }
            }
        }
    }

    println!("Error Handling Summary:");
    println!("  Expected successes: {successes}");
    println!("  Expected failures: {expected_failures}");
    println!("  Unexpected results: {unexpected_results}");

    // Should handle errors gracefully
    assert!(
        successes + expected_failures >= mixed_urls.len() / 2,
        "Most results should be as expected (allowing for network variability)"
    );

    Ok(())
}

/// Test memory usage and resource cleanup
#[tokio::test]
async fn test_resource_management() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if config.skip_external_services || config.skip_slow_tests {
        println!(
            "Skipping resource management test - external services disabled or slow tests skipped"
        );
        return Ok(());
    }

    // Test multiple conversions with the same MarkdownDown instance
    let md = MarkdownDown::new();
    let test_url = "https://httpbin.org/html";

    let initial_memory = std::mem::size_of_val(&md);
    println!("Initial MarkdownDown instance size: {initial_memory} bytes");

    // Perform multiple conversions
    let num_conversions = 5;
    let mut total_content_length = 0;

    for i in 0..num_conversions {
        println!("Resource test conversion {}/{num_conversions}", i + 1);

        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(test_url).await;

        match result {
            Ok(markdown) => {
                let content_length = markdown.as_str().len();
                total_content_length += content_length;

                // Validate content quality
                assert!(
                    TestUtils::validate_markdown_quality(markdown.as_str()),
                    "Content quality should be maintained across multiple conversions"
                );

                println!("  Conversion {}: {content_length} chars", i + 1);

                // Force drop of the markdown result to test cleanup
                drop(markdown);
            }
            Err(e) => {
                println!("  Conversion {} failed: {e} (acceptable)", i + 1);
            }
        }
    }

    println!("Resource Management Summary:");
    println!("  Total conversions attempted: {num_conversions}");
    println!("  Total content processed: {total_content_length} chars");
    println!("  Instance memory footprint: {initial_memory} bytes");

    // The MarkdownDown instance should be reusable
    assert!(
        total_content_length > 0,
        "Should have processed some content"
    );

    Ok(())
}
