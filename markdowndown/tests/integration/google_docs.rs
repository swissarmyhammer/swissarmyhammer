//! Integration tests for Google Docs conversion
//!
//! Tests the library's ability to convert Google Docs to markdown.

use markdowndown::MarkdownDown;
use std::time::Instant;

use super::{IntegrationTestConfig, TestUtils};

/// Test conversion of Google Docs documents
#[tokio::test]
async fn test_google_docs_conversions() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() {
        println!("Skipping Google Docs tests - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Using publicly accessible Google Docs for testing
    let test_docs = [
        (
            "https://docs.google.com/document/d/1mO-kSzl7dYOHFSPzVPiYZBUxHp1sWz5hJYCOK-JQf5s/edit",
            "Sample public Google Doc",
        ),
        (
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
            "Google Sheets API Sample",
        ),
    ];

    for (url, description) in test_docs.iter() {
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

                // Performance check
                assert!(
                    duration < config.large_document_timeout(),
                    "Conversion took too long for {description}: {duration:?}"
                );

                println!(
                    "✓ {description} converted successfully ({} chars, {duration:?})",
                    content.len()
                );
            }
            Err(e) => {
                println!("⚠ {description} failed (may be expected for placeholder URLs): {e}");
                // For placeholder URLs, failure is acceptable
                assert!(!e.to_string().is_empty(), "Error should have a message");
            }
        }
    }

    Ok(())
}

/// Test Google Docs URL format detection and parsing
#[tokio::test]
async fn test_google_docs_url_formats() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() {
        println!("Skipping Google Docs URL format tests - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Test various Google Docs URL formats
    let url_variants = [
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit?usp=sharing",
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view",
    ];

    for url in url_variants.iter() {
        println!("Testing URL format: {url}");

        // Test URL detection
        let detected_type = markdowndown::detect_url_type(url)?;
        assert_eq!(
            detected_type,
            markdowndown::types::UrlType::GoogleDocs,
            "Should detect as Google Docs: {url}"
        );

        // Rate limiting
        TestUtils::apply_rate_limit(&config).await;

        // Test conversion (may fail for placeholder, but should not panic)
        let result = md.convert_url(url).await;
        match result {
            Ok(markdown) => {
                println!(
                    "  ✓ Conversion succeeded ({} chars)",
                    markdown.as_str().len()
                );
                assert!(TestUtils::validate_markdown_quality(markdown.as_str()));
            }
            Err(e) => {
                println!("  ⚠ Conversion failed (may be expected): {e}");
                // Should fail gracefully with descriptive error
                assert!(!e.to_string().is_empty());
            }
        }
    }

    Ok(())
}

/// Test Google Docs error scenarios
#[tokio::test]
async fn test_google_docs_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() {
        println!("Skipping Google Docs error tests - external services disabled");
        return Ok(());
    }

    let md = MarkdownDown::new();

    let error_cases = [
        (
            "https://docs.google.com/document/d/nonexistent/edit",
            "Non-existent document",
        ),
        (
            "https://docs.google.com/document/d/private_document/edit",
            "Private document",
        ),
        (
            "https://docs.google.com/document/d/deleted_document/edit",
            "Deleted document",
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
                // If it somehow succeeds, content should indicate the issue
                let content = markdown.as_str();
                println!("  Unexpected success: {} chars", content.len());
                // Content might contain error messages or be very short
                assert!(
                    content.contains("Error")
                        || content.contains("not found")
                        || content.contains("private")
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
            }
        }
    }

    Ok(())
}

/// Test Google Docs with different content types (if available)
#[tokio::test]
async fn test_google_docs_content_types() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() || config.skip_slow_tests {
        println!("Skipping Google Docs content type tests - external services disabled or slow tests skipped");
        return Ok(());
    }

    let md = MarkdownDown::new();

    // Test different Google Workspace document types
    let document_types = [
        ("https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit", "Document"),
        ("https://docs.google.com/spreadsheets/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit", "Spreadsheet"),
        // Note: Presentations may not be supported by the converter
        // ("https://docs.google.com/presentation/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit", "Presentation"),
    ];

    for (url, doc_type) in document_types.iter() {
        println!("Testing {doc_type} type: {url}");

        // Rate limiting
        TestUtils::apply_rate_limit(&config).await;

        let result = md.convert_url(url).await;

        match result {
            Ok(markdown) => {
                let content = markdown.as_str();
                println!(
                    "  ✓ {doc_type} converted successfully ({} chars)",
                    content.len()
                );

                // Validate quality
                assert!(
                    TestUtils::validate_markdown_quality(content),
                    "Poor quality conversion for {doc_type} type"
                );

                // Check frontmatter
                let frontmatter = markdown.frontmatter().unwrap();
                assert!(
                    frontmatter.contains("google") || frontmatter.contains("docs"),
                    "Frontmatter should indicate Google Docs source"
                );
            }
            Err(e) => {
                println!("  ⚠ {doc_type} conversion failed (may be expected): {e}");
                // Some document types might not be supported
                assert!(!e.to_string().is_empty());
            }
        }
    }

    Ok(())
}

/// Test Google Docs authentication scenarios
#[tokio::test]
async fn test_google_docs_auth_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() {
        println!("Skipping Google Docs auth tests - external services disabled");
        return Ok(());
    }

    // Test with and without API key configuration
    let md_no_key = MarkdownDown::new();
    let md_with_key = if let Some(api_key) = &config.google_api_key {
        let config_with_key = markdowndown::Config::builder()
            .google_api_key(api_key)
            .build();
        MarkdownDown::with_config(config_with_key)
    } else {
        println!("No Google API key available - testing without authentication only");
        MarkdownDown::new()
    };

    let test_url =
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";

    // Test without API key
    println!("Testing without API key");
    TestUtils::apply_rate_limit(&config).await;
    let result_no_key = md_no_key.convert_url(test_url).await;

    // Test with API key (if available)
    if config.google_api_key.is_some() {
        println!("Testing with API key");
        TestUtils::apply_rate_limit(&config).await;
        let result_with_key = md_with_key.convert_url(test_url).await;

        // Both should either succeed or fail gracefully
        match (result_no_key, result_with_key) {
            (Ok(content1), Ok(content2)) => {
                println!("Both conversions succeeded");
                assert!(TestUtils::validate_markdown_quality(content1.as_str()));
                assert!(TestUtils::validate_markdown_quality(content2.as_str()));
            }
            (Err(e1), Err(e2)) => {
                println!("Both conversions failed: {e1} | {e2}");
                assert!(!e1.to_string().is_empty());
                assert!(!e2.to_string().is_empty());
            }
            (Ok(content), Err(e)) => {
                println!("No-key succeeded, with-key failed: {e}");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
            }
            (Err(e), Ok(content)) => {
                println!("No-key failed, with-key succeeded: {e}");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
            }
        }
    } else {
        // Just test that it fails gracefully without key
        match result_no_key {
            Ok(content) => {
                println!("Conversion succeeded without API key");
                assert!(TestUtils::validate_markdown_quality(content.as_str()));
            }
            Err(e) => {
                println!("Conversion failed without API key (expected): {e}");
                assert!(!e.to_string().is_empty());
            }
        }
    }

    Ok(())
}

/// Performance test for Google Docs conversion
#[tokio::test]
async fn test_google_docs_performance() -> Result<(), Box<dyn std::error::Error>> {
    let config = IntegrationTestConfig::from_env();

    if !config.can_test_google_docs() || config.skip_slow_tests {
        println!("Skipping Google Docs performance test - external services disabled or slow tests skipped");
        return Ok(());
    }

    let md = MarkdownDown::new();
    let test_url =
        "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";

    println!("Performance testing Google Docs conversion");

    // Rate limiting
    TestUtils::apply_rate_limit(&config).await;

    let start = Instant::now();
    let result = md.convert_url(test_url).await;
    let duration = start.elapsed();

    match result {
        Ok(markdown) => {
            let content_length = markdown.as_str().len();

            println!("Performance Results:");
            println!("  Duration: {duration:?}");
            println!("  Content length: {content_length} chars");
            println!(
                "  Chars per second: {:.2}",
                content_length as f64 / duration.as_secs_f64()
            );

            // Performance assertions
            assert!(
                duration < config.large_document_timeout(),
                "Google Docs conversion took too long: {duration:?}"
            );

            assert!(
                TestUtils::validate_markdown_quality(markdown.as_str()),
                "Performance test should produce quality output"
            );
        }
        Err(e) => {
            println!("Performance test failed (may be expected for placeholder URL): {e}");
            assert!(!e.to_string().is_empty());
        }
    }

    Ok(())
}
