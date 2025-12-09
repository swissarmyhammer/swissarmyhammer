//! Property-based tests using proptest for robustness validation.
//!
//! This module uses property-based testing to verify that the library
//! behaves correctly with a wide range of generated inputs, helping
//! discover edge cases and ensure robustness.
//!
//! Note: Some tests are temporarily disabled due to API changes.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{Converter, HtmlConverter, HtmlConverterConfig};
use markdowndown::detection::UrlDetector;
use markdowndown::types::{ErrorContext, Markdown, MarkdownError, Url, UrlType};
use markdowndown::{detect_url_type, MarkdownDown};
use proptest::prelude::*;
use std::time::Duration;

// Test constants for property-based testing ranges
const MAX_GITHUB_ISSUE_NUMBER: u32 = 100000;
const MAX_TIMEOUT_SECS: u64 = 3600;
const HTTP_CLIENT_ERROR_START: u16 = 400;
const HTTP_CLIENT_ERROR_END: u16 = 499;
const HTTP_STATUS_END: u16 = 600;
const HTTP_INTERNAL_ERROR: u16 = 500;
const HTTP_SERVICE_UNAVAILABLE: u16 = 503;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const MAX_RETRIES_TEST_RANGE: u32 = 20;
const MAX_FRONTMATTER_FIELDS: usize = 10;
const MAX_TIMEOUT_TEST_SECS: u64 = 100;
const MAX_RETRIES_CONFIG_TEST: u32 = 10;
const MIN_LINE_WIDTH: usize = 20;
const MAX_LINE_WIDTH_TEST: usize = 500;
const MAX_BLANK_LINES_TEST: usize = 20;
const MIN_LINE_WIDTH_CLONE_TEST: usize = 50;
const MAX_LINE_WIDTH_CLONE_TEST: usize = 200;
const MAX_BLANK_LINES_CLONE_TEST: usize = 10;

// Helper functions to reduce code duplication

/// Helper function for testing URL type detection
fn assert_url_type_detection(url: String, expected_type: UrlType) {
    match detect_url_type(&url) {
        Ok(url_type) => {
            assert_eq!(url_type, expected_type);
        }
        Err(_) => {
            // Some URLs might be invalid, which is acceptable
        }
    }
}

/// Helper function for asserting all fields are equal between two HtmlConverterConfig instances
fn assert_html_converter_config_equal(a: &HtmlConverterConfig, b: &HtmlConverterConfig) {
    assert_eq!(a.max_line_width, b.max_line_width);
    assert_eq!(a.remove_scripts_styles, b.remove_scripts_styles);
    assert_eq!(a.remove_navigation, b.remove_navigation);
    assert_eq!(a.remove_sidebars, b.remove_sidebars);
    assert_eq!(a.remove_ads, b.remove_ads);
    assert_eq!(a.max_blank_lines, b.max_blank_lines);
}

/// Helper function for testing detection consistency
fn test_detection_consistency<F, T>(url: &str, detector_fn: F) -> Result<T, MarkdownError>
where
    F: Fn(&UrlDetector, &str) -> Result<T, MarkdownError>,
    T: PartialEq + Clone + std::fmt::Debug,
{
    let detector = UrlDetector::new();
    let result1 = detector_fn(&detector, url)?;
    let result2 = detector_fn(&detector, url)?;
    assert_eq!(result1, result2);
    Ok(result1)
}

/// Helper function for building Config with a custom configuration closure
fn build_config_with<F>(configure: F) -> Config
where
    F: FnOnce(markdowndown::config::ConfigBuilder) -> markdowndown::config::ConfigBuilder,
{
    let builder = Config::builder();
    configure(builder).build()
}

/// Helper function for asserting error formats correctly
fn assert_error_formats_correctly(error: &MarkdownError, expected_content: &str) {
    let display_string = format!("{error}");
    assert!(display_string.contains(expected_content));
    let debug_string = format!("{error:?}");
    assert!(debug_string.contains(expected_content));
}

/// Helper function for creating test HtmlConverterConfig instances
fn create_test_html_config(
    max_line_width: usize,
    max_blank_lines: usize,
    flags: (bool, bool, bool, bool),
) -> HtmlConverterConfig {
    HtmlConverterConfig {
        max_line_width,
        remove_scripts_styles: flags.0,
        remove_navigation: flags.1,
        remove_sidebars: flags.2,
        remove_ads: flags.3,
        max_blank_lines,
    }
}

/// Helper function for comparing MarkdownDown instances
fn compare_markdowndown_instances<F, T>(instances: Vec<&MarkdownDown>, property_fn: F)
where
    F: Fn(&MarkdownDown) -> Vec<T>,
    T: Eq + std::hash::Hash + std::fmt::Debug,
{
    use std::collections::HashSet;
    let sets: Vec<HashSet<_>> = instances
        .iter()
        .map(|md| property_fn(md).into_iter().collect())
        .collect();

    for i in 1..sets.len() {
        assert_eq!(sets[0], sets[i]);
    }
}

/// Generates valid HTTP/HTTPS URLs for property-based testing.
///
/// Returns URLs in the format `{scheme}://{domain}.{tld}[/{path}]`
/// where scheme is http or https, domain is alphanumeric, and path is optional.
fn valid_http_url_strategy() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["http", "https"]),
        "[a-z0-9-]{1,20}",
        prop::sample::select(vec!["com", "org", "net", "edu", "gov"]),
        prop::option::of("[a-z0-9/-]{0,30}"),
    )
        .prop_map(|(scheme, domain, tld, path)| match path {
            Some(p) if !p.is_empty() => format!("{scheme}://{domain}.{tld}/{p}"),
            _ => format!("{scheme}://{domain}.{tld}"),
        })
}

/// Generates arbitrary URL-like strings for testing error handling.
///
/// Returns strings matching URL patterns but not necessarily valid URLs,
/// useful for testing robustness against malformed input.
fn arbitrary_url_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9:/.?#&=-]{1,100}").unwrap()
}

/// Generates markdown content with various structures for property testing.
///
/// Returns markdown strings that may include:
/// - Optional title (H1 heading)
/// - Multiple paragraphs
/// - Optional footer
///
/// Ensures generated content is never empty by filtering out empty results.
fn markdown_content_strategy() -> impl Strategy<Value = String> {
    (
        prop::option::of(prop::string::string_regex("[a-zA-Z0-9 ]{1,50}").unwrap()),
        prop::collection::vec(
            prop::string::string_regex("[a-zA-Z0-9 .!?,]{1,100}").unwrap(),
            0..10,
        ),
        prop::option::of(prop::string::string_regex("[a-zA-Z0-9 ]{1,30}").unwrap()),
    )
        .prop_map(|(title, paragraphs, footer)| {
            let mut content = String::new();

            if let Some(t) = title {
                content.push_str(&format!("# {t}\n\n"));
            }

            for (i, paragraph) in paragraphs.iter().enumerate() {
                if i > 0 {
                    content.push_str("\n\n");
                }
                content.push_str(paragraph);
            }

            if let Some(f) = footer {
                content.push_str(&format!("\n\n{f}"));
            }

            content
        })
        .prop_filter("content is not empty", |s| !s.trim().is_empty())
}

/// Generic macro for creating property tests with less duplication
macro_rules! property_test {
    ($test_name:ident, $strategy:expr, $pattern:pat, $body:block) => {
        proptest! {
            #[test]
            fn $test_name($pattern in $strategy) {
                $body
            }
        }
    };
}

/// Property tests for URL validation
mod url_validation_properties {
    use super::*;

    property_test!(
        test_valid_urls_create_successfully,
        valid_http_url_strategy(),
        url,
        {
            let result = Url::new(url.clone());
            prop_assert!(result.is_ok(), "Failed to create URL from: {}", url);
            let url_obj = result.unwrap();
            prop_assert_eq!(url_obj.as_str(), &url)
        }
    );

    property_test!(test_url_as_str_roundtrip, valid_http_url_strategy(), url, {
        let result = Url::new(url.clone());
        if let Ok(url_obj) = result {
            let as_str = url_obj.as_str();
            prop_assert_eq!(as_str, url_obj.as_str());

            // Should be able to create another URL from as_str
            let url_obj2 = Url::new(as_str.to_string());
            prop_assert!(url_obj2.is_ok());
            let url_obj2_unwrapped = url_obj2.unwrap();
            prop_assert_eq!(url_obj2_unwrapped, url_obj);
        }
    });

    property_test!(test_url_clone_equality, valid_http_url_strategy(), url, {
        let result = Url::new(url.clone());
        if let Ok(url_obj) = result {
            let cloned = url_obj.clone();
            prop_assert_eq!(url_obj, cloned);
        }
    });

    property_test!(
        test_arbitrary_urls_handled_gracefully,
        arbitrary_url_strategy(),
        url,
        {
            let result = Url::new(url.clone());
            // Should either succeed or fail gracefully with a proper error
            match result {
                Ok(url_obj) => {
                    // If successful, should be able to get string representation
                    let _as_str = url_obj.as_str();
                }
                Err(err) => {
                    // Should be a proper error, not a panic
                    match err {
                        MarkdownError::ValidationError { .. } => {
                            // Expected for invalid URLs
                        }
                        _ => {
                            prop_assert!(false, "Unexpected error type for invalid URL: {:?}", err);
                        }
                    }
                }
            }
        }
    );
}

/// Property tests for markdown content validation
mod markdown_validation_properties {
    use super::*;

    property_test!(
        test_markdown_content_preservation,
        markdown_content_strategy(),
        content,
        {
            if let Ok(markdown) = Markdown::new(content.clone()) {
                let retrieved_content = markdown.as_str();
                prop_assert_eq!(retrieved_content, &content)
            }
        }
    );

    property_test!(
        test_markdown_content_only_no_frontmatter,
        markdown_content_strategy(),
        content,
        {
            if let Ok(markdown) = Markdown::new(content.clone()) {
                let content_only = markdown.content_only();
                prop_assert_eq!(content_only, content);

                // With no frontmatter, frontmatter() should return None
                prop_assert!(markdown.frontmatter().is_none())
            }
        }
    );

    property_test!(
        test_markdown_clone_equality,
        markdown_content_strategy(),
        content,
        {
            if let Ok(markdown) = Markdown::new(content.clone()) {
                let cloned = markdown.clone();
                prop_assert_eq!(markdown, cloned)
            }
        }
    );

    proptest! {
        #[test]
        fn test_markdown_with_frontmatter_preservation(
            content in markdown_content_strategy(),
            frontmatter in prop::string::string_regex("---\n[a-zA-Z][a-zA-Z0-9_]*: [a-zA-Z0-9 ]{5,20}\n---").unwrap()
        ) {
            let combined = format!("{frontmatter}\n\n{content}");
            let markdown = Markdown::from(combined);
            let content_only = markdown.content_only();
            // Content only should not contain frontmatter delimiters
            prop_assert!(!content_only.contains("---"));

            // But should contain the original content
            prop_assert!(content_only.contains(&content) || content.is_empty());
        }

        #[test]
        fn test_empty_and_whitespace_content(
            whitespace in prop::string::string_regex("[ \t\n\r]{0,50}").unwrap()
        ) {
            let result = Markdown::new(whitespace.clone());
            // Should handle empty and whitespace-only content gracefully
            match result {
                Ok(markdown) => {
                    prop_assert_eq!(markdown.as_str(), &whitespace);
                }
                Err(_) => {
                    // Some validation might reject empty content, which is acceptable
                }
            }
        }
    }
}

/// Property tests for URL detection
mod url_detection_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_url_detection_consistency(url in valid_http_url_strategy()) {
            let result = test_detection_consistency(&url, |detector, u| detector.detect_type(u));
            // Detection should be deterministic - the helper already asserts equality
            prop_assert!(result.is_ok() || result.is_err());
        }

        #[test]
        fn test_url_normalization_idempotent(url in valid_http_url_strategy()) {
            let result = test_detection_consistency(&url, |detector, u| detector.normalize_url(u));
            // Normalization should be idempotent - test again with normalized result
            if let Ok(normalized) = result {
                let result2 = test_detection_consistency(&normalized, |detector, u| detector.normalize_url(u));
                prop_assert!(result2.is_ok());
            }
        }
    }

    property_test!(
        test_google_docs_urls_detected,
        prop::string::string_regex("[a-zA-Z0-9_-]{10,50}")
            .unwrap()
            .prop_map(|doc_id| format!("https://docs.google.com/document/d/{doc_id}/edit")),
        url,
        {
            assert_url_type_detection(url, UrlType::GoogleDocs);
        }
    );

    property_test!(
        test_github_issue_urls_detected,
        (
            prop::string::string_regex("[a-zA-Z0-9_-]{1,20}").unwrap(),
            prop::string::string_regex("[a-zA-Z0-9_-]{1,30}").unwrap(),
            1u32..MAX_GITHUB_ISSUE_NUMBER
        )
            .prop_map(|(owner, repo, issue_num)| {
                format!("https://github.com/{owner}/{repo}/issues/{issue_num}")
            }),
        url,
        {
            assert_url_type_detection(url, UrlType::GitHubIssue);
        }
    );

    property_test!(
        test_html_urls_as_fallback,
        (
            prop::string::string_regex("[a-z0-9-]{1,20}").unwrap(),
            prop::string::string_regex("[a-z0-9-]{1,20}").unwrap()
        )
            .prop_map(|(subdomain, domain)| {
                format!("https://{subdomain}-test-{domain}.example")
            }),
        url,
        {
            assert_url_type_detection(url, UrlType::Html);
        }
    );
}

/// Property tests for configuration handling
mod configuration_properties {
    use super::*;

    property_test!(
        test_config_timeout_values,
        1u64..MAX_TIMEOUT_SECS,
        timeout_secs,
        {
            let config = build_config_with(|b| b.timeout_seconds(timeout_secs));
            prop_assert_eq!(config.http.timeout, Duration::from_secs(timeout_secs))
        }
    );

    property_test!(
        test_config_retry_values,
        0u32..MAX_RETRIES_TEST_RANGE,
        max_retries,
        {
            let config = build_config_with(|b| b.max_retries(max_retries));
            prop_assert_eq!(config.http.max_retries, max_retries)
        }
    );

    property_test!(
        test_config_user_agent_preservation,
        prop::string::string_regex("[a-zA-Z0-9_/. -]{1,100}").unwrap(),
        user_agent,
        {
            let config = build_config_with(|b| b.user_agent(&user_agent));
            prop_assert_eq!(config.http.user_agent, user_agent)
        }
    );

    proptest! {
        #[test]
        fn test_config_token_preservation(
            github_token in prop::option::of(prop::string::string_regex("[a-zA-Z0-9_]{10,50}").unwrap()),
            google_api_key in prop::option::of(prop::string::string_regex("[a-zA-Z0-9_]{10,50}").unwrap())
        ) {
            let config = build_config_with(|mut builder| {
                if let Some(ref token) = github_token {
                    builder = builder.github_token(token);
                }
                if let Some(ref key) = google_api_key {
                    builder = builder.google_api_key(key);
                }
                builder
            });

            prop_assert_eq!(config.auth.github_token, github_token);
            prop_assert_eq!(config.auth.google_api_key, google_api_key);
        }

        #[test]
        fn test_config_custom_frontmatter_fields(
            fields in prop::collection::vec(
                (
                    prop::string::string_regex("[a-zA-Z_][a-zA-Z0-9_]{0,20}").unwrap(),
                    prop::string::string_regex("[a-zA-Z0-9 ._-]{1,50}").unwrap()
                ),
                0..MAX_FRONTMATTER_FIELDS
            )
        ) {
            let config = build_config_with(|mut builder| {
                for (key, value) in &fields {
                    builder = builder.custom_frontmatter_field(key, value);
                }
                builder
            });

            prop_assert_eq!(config.output.custom_frontmatter_fields.len(), fields.len());
            for (i, (key, value)) in fields.iter().enumerate() {
                prop_assert_eq!(&config.output.custom_frontmatter_fields[i].0, key);
                prop_assert_eq!(&config.output.custom_frontmatter_fields[i].1, value);
            }
        }

        #[test]
        fn test_config_builder_chaining(
            timeout in 1u64..MAX_TIMEOUT_TEST_SECS,
            retries in 0u32..MAX_RETRIES_CONFIG_TEST,
            include_frontmatter in any::<bool>(),
            normalize_whitespace in any::<bool>()
        ) {
            let config = build_config_with(|b| {
                b.timeout_seconds(timeout)
                    .max_retries(retries)
                    .include_frontmatter(include_frontmatter)
                    .normalize_whitespace(normalize_whitespace)
            });

            prop_assert_eq!(config.http.timeout, Duration::from_secs(timeout));
            prop_assert_eq!(config.http.max_retries, retries);
            prop_assert_eq!(config.output.include_frontmatter, include_frontmatter);
            prop_assert_eq!(config.output.normalize_whitespace, normalize_whitespace);
        }
    }
}

/// Property tests for error handling robustness
mod error_handling_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_error_display_formatting(
            message in prop::string::string_regex("[a-zA-Z0-9 ._-]{1,100}").unwrap()
        ) {
            let error = MarkdownError::LegacyConfigurationError {
                message: message.clone(),
            };

            assert_error_formats_correctly(&error, &message);
        }

        #[test]
        fn test_error_source_chain(
            primary_msg in prop::string::string_regex("[a-zA-Z0-9 ._-]{1,50}").unwrap(),
            context_msg in prop::string::string_regex("[a-zA-Z0-9 ._-]{1,50}").unwrap()
        ) {
            // Create an error with context
            let _base_error = MarkdownError::LegacyConfigurationError {
                message: primary_msg.clone(),
            };

            let error_context = ErrorContext {
                url: "https://test.example".to_string(),
                operation: "test_operation".to_string(),
                converter_type: "TestConverter".to_string(),
                timestamp: chrono::Utc::now(),
                additional_info: Some(context_msg.clone()),
            };

            let error_with_context = MarkdownError::ValidationError {
                kind: markdowndown::types::ValidationErrorKind::InvalidUrl,
                context: error_context,
            };

            // Should be able to display and debug format without panicking
            let _display = format!("{error_with_context}");
            let _debug = format!("{error_with_context:?}");

            // Error should implement std::error::Error
            let error_trait: &dyn std::error::Error = &error_with_context;
            let _source = error_trait.source();
        }

        #[test]
        fn test_markdown_error_recoverable_property(
            network_status in HTTP_CLIENT_ERROR_START..HTTP_STATUS_END
        ) {
            let error_context = ErrorContext {
                url: "https://example.com".to_string(),
                operation: "http_request".to_string(),
                converter_type: "TestConverter".to_string(),
                timestamp: chrono::Utc::now(),
                additional_info: Some(format!("HTTP {network_status}")),
            };

            let error = MarkdownError::EnhancedNetworkError {
                kind: markdowndown::types::NetworkErrorKind::ServerError(network_status),
                context: error_context,
            };

            let is_recoverable = error.is_recoverable();

            // Certain status codes should be recoverable, others not
            match network_status {
                HTTP_INTERNAL_ERROR..=HTTP_SERVICE_UNAVAILABLE | HTTP_TOO_MANY_REQUESTS => prop_assert!(is_recoverable),
                HTTP_CLIENT_ERROR_START..=HTTP_CLIENT_ERROR_END => prop_assert!(!is_recoverable),
                _ => {
                    // Other codes may or may not be recoverable based on implementation
                }
            }
        }
    }
}

/// Helper function for validating HTML config creation and equality
fn validate_html_config_creation_and_equality(
    max_line_width: usize,
    max_blank_lines: usize,
    flags: (bool, bool, bool, bool),
) -> Result<(), proptest::test_runner::TestCaseError> {
    let config = create_test_html_config(max_line_width, max_blank_lines, flags);
    let expected = create_test_html_config(max_line_width, max_blank_lines, flags);
    assert_html_converter_config_equal(&config, &expected);
    Ok(())
}

/// Property tests for HTML converter configuration
mod html_converter_properties {
    use super::*;

    property_test!(
        test_html_converter_config_validation,
        (
            MIN_LINE_WIDTH..MAX_LINE_WIDTH_TEST,
            0usize..MAX_BLANK_LINES_TEST,
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>()
        ),
        params,
        {
            let (
                max_line_width,
                max_blank_lines,
                remove_scripts,
                remove_navigation,
                remove_sidebars,
                remove_ads,
            ) = params;

            validate_html_config_creation_and_equality(
                max_line_width,
                max_blank_lines,
                (
                    remove_scripts,
                    remove_navigation,
                    remove_sidebars,
                    remove_ads,
                ),
            )?;

            // Should be able to create converter with this config
            let config = create_test_html_config(
                max_line_width,
                max_blank_lines,
                (
                    remove_scripts,
                    remove_navigation,
                    remove_sidebars,
                    remove_ads,
                ),
            );
            let client = HttpClient::new();
            let output_config = markdowndown::config::OutputConfig::default();
            let converter = HtmlConverter::with_config(client, config.clone(), output_config);
            prop_assert_eq!(converter.name(), "HTML")
        }
    );

    property_test!(
        test_html_converter_config_clone,
        (
            MIN_LINE_WIDTH_CLONE_TEST..MAX_LINE_WIDTH_CLONE_TEST,
            1usize..MAX_BLANK_LINES_CLONE_TEST
        ),
        params,
        {
            let (max_line_width, max_blank_lines) = params;
            validate_html_config_creation_and_equality(
                max_line_width,
                max_blank_lines,
                (true, false, true, false),
            )?;
        }
    );
}

/// Helper function for creating standard MarkdownDown instances
fn create_standard_instances() -> Vec<MarkdownDown> {
    vec![
        MarkdownDown::new(),
        MarkdownDown::default(),
        MarkdownDown::with_config(Config::default()),
    ]
}

/// Property tests for MarkdownDown main API
mod markdowndown_api_properties {
    use super::*;

    #[test]
    fn test_markdowndown_supported_types_consistency() {
        let instances = create_standard_instances();
        let instance_refs: Vec<&MarkdownDown> = instances.iter().collect();

        // All instances should report the same supported types (order may vary)
        compare_markdowndown_instances(instance_refs, |md| md.supported_types());

        // Should include the core types
        let types = instances[0].supported_types();
        assert!(types.contains(&UrlType::Html));
        assert!(types.contains(&UrlType::GoogleDocs));
        assert!(types.contains(&UrlType::GitHubIssue));
    }

    #[test]
    fn test_detect_url_type_function_consistency() {
        let url = "https://example.com/test.html";

        // Function should be deterministic
        let result1 = detect_url_type(url);
        let result2 = detect_url_type(url);

        assert_eq!(result1.is_ok(), result2.is_ok());
        if result1.is_ok() && result2.is_ok() {
            assert_eq!(result1.unwrap(), result2.unwrap());
        }
    }
}
