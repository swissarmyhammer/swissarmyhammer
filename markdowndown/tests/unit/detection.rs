//! Comprehensive unit tests for URL detection and classification.
//!
//! This module tests URL pattern matching, edge cases, validation,
//! normalization, and all supported URL types with thorough coverage.

use markdowndown::detection::UrlDetector;
use markdowndown::types::{MarkdownError, UrlType};
use proptest::prelude::*;

// Test configuration constants
// Test with 2000 chars to verify handling of URLs near typical browser limits
const TEST_VERY_LONG_PATH_LENGTH: usize = 2000;
// DNS RFC 1035 allows max 63 chars, testing near that limit
const MAX_DOMAIN_LABEL_LENGTH: usize = 61;
// Minimum valid TLD length per IANA standards
const MIN_TLD_LENGTH: usize = 2;
// Typical maximum TLD length for testing (e.g., 'museum')
const MAX_TLD_LENGTH: usize = 6;

// Shared test URL constants to avoid duplication across tests
const GOOGLE_DOCS_URLS: &[&str] = &[
    "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
    "https://docs.google.com/document/d/abc123def456/view",
    "https://docs.google.com/document/d/test123/edit#heading=h.123",
    "https://drive.google.com/file/d/1234567890abcdef/view",
    "https://drive.google.com/file/d/xyz789/edit",
];

const GITHUB_ISSUE_URLS: &[&str] = &[
    "https://github.com/owner/repo/issues/123",
    "https://github.com/microsoft/vscode/issues/42",
    "https://github.com/rust-lang/rust/issues/12345",
    "https://github.com/owner/repo/pull/456",
    "https://github.com/microsoft/vscode/pull/789",
    "https://github.com/rust-lang/rust/pull/98765",
    "https://github.com/owner/repo/issues/1",
    "https://github.com/owner/repo/pull/999999",
];

const HTML_URLS: &[&str] = &[
    "https://example.com",
    "https://www.example.com/page.html",
    "https://blog.example.com/post/123",
    "https://news.example.org/article?id=456",
    "https://www.wikipedia.org/wiki/Rust_(programming_language)",
    "https://stackoverflow.com/questions/12345/how-to-do-something",
    "https://reddit.com/r/rust/comments/abc123/title",
    "https://github.com/owner/repo",
    "https://github.com/owner/repo/commits",
    "https://github.com/owner/repo/tree/main",
];

const VALID_TEST_URLS: &[&str] = &[
    "https://example.com",
    "http://example.com",
    "https://www.example.com",
    "https://subdomain.example.com",
    "https://example.com/path",
    "https://example.com/path/to/resource",
    "https://example.com:8080",
    "https://example.com:8080/path",
    "https://example.com/path?query=value",
    "https://example.com/path?query=value#fragment",
    "https://192.168.1.1",
    "https://localhost:3000",
    "https://user:pass@example.com",
    "https://example.com/path/with-dashes_and_underscores",
];

const INVALID_TEST_URLS: &[&str] = &[
    "not-a-url",
    "ftp://example.com",
    "mailto:test@example.com",
    "javascript:alert('xss')",
    "data:text/html,<h1>Test</h1>",
    "",
    "   ",
    "example.com",
    "www.example.com",
    "//example.com",
    "https://",
    "http://",
];

mod helpers {
    use super::*;

    /// Creates a new URL detector instance for testing.
    pub fn create_detector() -> UrlDetector {
        UrlDetector::new()
    }

    /// Returns a collection of sample URLs grouped by their expected URL type for testing type detection.
    pub fn sample_urls_by_type() -> Vec<(UrlType, Vec<&'static str>)> {
        vec![
            (UrlType::GoogleDocs, GOOGLE_DOCS_URLS.to_vec()),
            (UrlType::GitHubIssue, GITHUB_ISSUE_URLS.to_vec()),
            (UrlType::Html, HTML_URLS.to_vec()),
        ]
    }

    /// Asserts that all provided URLs are detected as the expected URL type.
    ///
    /// # Panics
    ///
    /// Panics if any URL doesn't match the expected type.
    pub fn assert_urls_match_type(urls: &[&str], expected_type: UrlType) {
        assert_urls_match_type_with_detector(&create_detector(), urls, expected_type);
    }

    /// Asserts that all provided URLs are detected as the expected URL type using a provided detector.
    ///
    /// # Panics
    ///
    /// Panics if any URL doesn't match the expected type.
    pub fn assert_urls_match_type_with_detector(detector: &UrlDetector, urls: &[&str], expected_type: UrlType) {
        for url in urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, expected_type, "Failed for URL: {url}");
        }
    }

    /// Asserts that URL normalization produces expected results for test cases.
    ///
    /// # Panics
    ///
    /// Panics if any normalization result doesn't match the expected value.
    pub fn assert_normalization(test_cases: &[(&str, &str)]) {
        assert_normalization_with_detector(&create_detector(), test_cases);
    }

    /// Asserts that URL normalization produces expected results for test cases using a provided detector.
    ///
    /// # Panics
    ///
    /// Panics if any normalization result doesn't match the expected value.
    pub fn assert_normalization_with_detector(detector: &UrlDetector, test_cases: &[(&str, &str)]) {
        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(result, *expected, "Failed to normalize: {input}");
        }
    }

    /// Asserts URL validation results using a custom validator function.
    ///
    /// # Panics
    ///
    /// Panics based on the validator function's assertions.
    pub fn assert_urls_validation<F>(urls: &[&str], validator: F)
    where
        F: Fn(&str, Result<(), MarkdownError>),
    {
        let detector = create_detector();
        for url in urls {
            let result = detector.validate_url(url);
            validator(url, result);
        }
    }

    /// Asserts that all provided URLs pass validation.
    ///
    /// # Panics
    ///
    /// Panics if any URL fails validation.
    pub fn assert_urls_valid(urls: &[&str]) {
        assert_urls_validation(urls, |url, result| {
            assert!(result.is_ok(), "Should validate URL: {url}");
        });
    }

    /// Asserts that all provided URLs fail validation with InvalidUrl error.
    ///
    /// # Panics
    ///
    /// Panics if any URL passes validation or fails with a different error type.
    pub fn assert_urls_invalid(urls: &[&str]) {
        assert_urls_validation(urls, |url, result| {
            assert!(result.is_err(), "Should reject URL: {url}");
            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, .. } => {
                    assert_eq!(kind, markdowndown::types::ValidationErrorKind::InvalidUrl);
                }
                _ => panic!("Expected InvalidUrl error for: {url}"),
            }
        });
    }

    /// Asserts that all provided URLs are detected as HTML type.
    ///
    /// # Panics
    ///
    /// Panics if any URL is not detected as HTML type.
    pub fn assert_urls_are_html(urls: &[&str]) {
        assert_urls_match_type(urls, UrlType::Html);
    }

    /// Tests URLs using a custom validator closure for type detection results.
    ///
    /// # Panics
    ///
    /// Panics based on the validator function's assertions.
    pub fn test_urls_with<F>(urls: &[&str], validator: F)
    where
        F: Fn(&str, Result<UrlType, MarkdownError>),
    {
        let detector = create_detector();
        for url in urls {
            let result = detector.detect_type(url);
            validator(url, result);
        }
    }

    /// Asserts that URL normalization is idempotent (stable across multiple normalizations)
    /// and that type detection remains consistent before and after normalization.
    ///
    /// # Panics
    ///
    /// Panics if normalization isn't stable or type detection changes.
    pub fn assert_roundtrip_stable(url: &str, detector: &UrlDetector) {
        let normalized1 = detector.normalize_url(url).unwrap();
        let normalized2 = detector.normalize_url(&normalized1).unwrap();
        let normalized3 = detector.normalize_url(&normalized2).unwrap();

        assert_eq!(normalized1, normalized2);
        assert_eq!(normalized2, normalized3);

        let type1 = detector.detect_type(url).unwrap();
        let type2 = detector.detect_type(&normalized1).unwrap();
        let type3 = detector.detect_type(&normalized2).unwrap();

        assert_eq!(type1, type2);
        assert_eq!(type2, type3);
    }

    /// Asserts that query parameters in URLs match expected values.
    ///
    /// # Panics
    ///
    /// Panics if tracking parameters aren't removed or expected parameters don't match.
    pub fn assert_query_params(result_url: &str, expected_url: &str) {
        let result_parsed = url::Url::parse(result_url).unwrap();
        let expected_parsed = url::Url::parse(expected_url).unwrap();
        
        // Verify tracking parameters are removed
        for (key, _) in result_parsed.query_pairs() {
            assert!(!key.starts_with("utm_"), "Tracking parameter not removed: {}", key);
            assert!(key != "ref", "Tracking parameter 'ref' not removed");
        }
        
        // Verify important parameters are preserved
        let result_params: std::collections::HashMap<_, _> = result_parsed.query_pairs().collect();
        let expected_params: std::collections::HashMap<_, _> = expected_parsed.query_pairs().collect();
        
        for (key, value) in expected_params {
            assert_eq!(
                result_params.get(&key),
                Some(&value),
                "Expected parameter '{}' with value '{}' not found",
                key,
                value
            );
        }
    }

    /// Tests URL type detection with extensions for multiple URL sets.
    ///
    /// # Panics
    ///
    /// Panics if any URL doesn't match the expected type.
    pub fn test_url_type_with_extensions(
        base_urls: &[&str],
        extended_urls: &[&str],
        expected_type: UrlType,
    ) {
        assert_urls_match_type(base_urls, expected_type);
        assert_urls_match_type(extended_urls, expected_type);
    }

    /// Tests GitHub URL classification with table-driven approach.
    ///
    /// # Panics
    ///
    /// Panics if any URL doesn't match the expected type.
    pub fn test_github_url_classification(test_cases: &[(&str, UrlType, &str)]) {
        let detector = create_detector();
        for (url, expected_type, description) in test_cases {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, *expected_type, "{}: Failed for URL: {url}", description);
        }
    }
}

/// Tests for URL detector creation and basic functionality
mod detector_creation_tests {
    use super::*;

    #[test]
    fn test_url_detector_new() {
        let _detector = UrlDetector::new();
        // Detector should be created successfully
        // We can't test private fields directly, so test through behavior
        // Test passes if no panic occurs during creation
    }

    #[test]
    fn test_url_detector_default() {
        let _detector = UrlDetector::default();
        // Default should be equivalent to new()
        // Test passes if no panic occurs during creation
    }
}

/// Tests for URL type detection
mod url_type_detection_tests {
    use super::*;

    #[test]
    fn test_detect_all_supported_types() {
        let detector = helpers::create_detector();

        for (expected_type, urls) in helpers::sample_urls_by_type() {
            for url in urls {
                let result = detector.detect_type(url);
                assert!(result.is_ok(), "Detection failed for URL: {url}");
                assert_eq!(
                    result.unwrap(),
                    expected_type,
                    "Wrong type detected for URL: {url}"
                );
            }
        }
    }

    #[test]
    fn test_google_docs_url_detection() {
        let extended_google_docs = [
            "https://docs.google.com/document/d/abc123/view",
            "https://docs.google.com/document/d/test_doc_id_123/edit#heading=h.xyz",
            "https://docs.google.com/document/d/short/copy",
            "https://docs.google.com/document/d/1234567890/edit?usp=sharing",
            "https://drive.google.com/file/d/test_file/preview",
            "https://drive.google.com/file/d/abc123def456/view?usp=sharing",
        ];

        helpers::test_url_type_with_extensions(
            GOOGLE_DOCS_URLS,
            &extended_google_docs,
            UrlType::GoogleDocs,
        );
    }

    #[test]
    fn test_github_issue_url_detection() {
        let extended_github_issues = [
            "https://github.com/facebook/react/issues/1",
            "https://github.com/nodejs/node/issues/999999",
            "https://github.com/facebook/react/pull/1",
            "https://github.com/nodejs/node/pull/999999",
        ];

        helpers::test_url_type_with_extensions(
            GITHUB_ISSUE_URLS,
            &extended_github_issues,
            UrlType::GitHubIssue,
        );
    }

    #[test]
    fn test_html_url_detection() {
        let extended_html = [
            "https://stackoverflow.com/questions/12345",
            "https://reddit.com/r/rust",
            "https://www.wikipedia.org/wiki/Main_Page",
            "https://github.com/owner/repo/blob/main/README.md",
        ];

        helpers::test_url_type_with_extensions(HTML_URLS, &extended_html, UrlType::Html);
    }
}

/// Tests for GitHub URL edge cases
mod github_edge_cases {
    use super::*;

    #[test]
    fn test_github_url_classification() {
        let test_cases = [
            // URLs with components (fragments and query parameters)
            ("https://github.com/owner/repo/issues/123#issuecomment-456789", UrlType::GitHubIssue, "issue with comment fragment"),
            ("https://github.com/microsoft/vscode/issues/42#event-123456", UrlType::GitHubIssue, "issue with event fragment"),
            ("https://github.com/rust-lang/rust/pull/12345#pullrequestreview-789", UrlType::GitHubIssue, "pull with review fragment"),
            ("https://github.com/owner/repo/pull/456#discussion_r123456789", UrlType::GitHubIssue, "pull with discussion fragment"),
            ("https://github.com/owner/repo/issues/123?tab=timeline", UrlType::GitHubIssue, "issue with tab query param"),
            ("https://github.com/microsoft/vscode/pull/456?diff=unified", UrlType::GitHubIssue, "pull with diff query param"),
            ("https://github.com/rust-lang/rust/issues/789?q=is%3Aissue+is%3Aopen", UrlType::GitHubIssue, "issue with search query param"),
            
            // Non-issue/pull URLs
            ("https://github.com/owner/repo", UrlType::Html, "repository home"),
            ("https://github.com/owner/repo/issues", UrlType::Html, "issues list page"),
            ("https://github.com/owner/repo/pull", UrlType::Html, "pulls list page"),
            ("https://github.com/owner/repo/commits", UrlType::Html, "commits page"),
            ("https://github.com/owner/repo/tree/main", UrlType::Html, "tree view"),
            ("https://github.com/owner/repo/blob/main/README.md", UrlType::Html, "blob view"),
            ("https://github.com/owner/repo/releases", UrlType::Html, "releases page"),
            ("https://github.com/owner/repo/wiki", UrlType::Html, "wiki page"),
            ("https://github.com/owner/repo/settings", UrlType::Html, "settings page"),
            ("https://github.com/owner/repo/actions", UrlType::Html, "actions page"),
            ("https://github.com/owner/repo/issues/abc", UrlType::Html, "issue with non-numeric id"),
            ("https://github.com/owner/repo/pull/def", UrlType::Html, "pull with non-numeric id"),
            ("https://github.com/owner/repo/issues/", UrlType::Html, "issue with empty id"),
            ("https://github.com/owner/repo/pull/", UrlType::Html, "pull with empty id"),
            
            // Valid issue numbers
            ("https://github.com/owner/repo/issues/1", UrlType::GitHubIssue, "issue with minimal id"),
            ("https://github.com/owner/repo/issues/123", UrlType::GitHubIssue, "issue with medium id"),
            ("https://github.com/owner/repo/issues/999999", UrlType::GitHubIssue, "issue with large id"),
            ("https://github.com/owner/repo/pull/1", UrlType::GitHubIssue, "pull with minimal id"),
            ("https://github.com/owner/repo/pull/123", UrlType::GitHubIssue, "pull with medium id"),
            ("https://github.com/owner/repo/pull/999999", UrlType::GitHubIssue, "pull with large id"),
            
            // Invalid issue numbers
            ("https://github.com/owner/repo/issues/123abc", UrlType::Html, "issue id with trailing letters"),
            ("https://github.com/owner/repo/issues/abc123", UrlType::Html, "issue id with leading letters"),
            ("https://github.com/owner/repo/pull/xyz", UrlType::Html, "pull id with only letters"),
            ("https://github.com/owner/repo/pull/123xyz", UrlType::Html, "pull id with trailing letters"),
            ("https://github.com/owner/repo/pull/xyz123", UrlType::Html, "pull id with leading letters"),
            
            // Valid path structures
            ("https://github.com/a/b/issues/1", UrlType::GitHubIssue, "minimal owner and repo names"),
            ("https://github.com/owner-name/repo-name/issues/123", UrlType::GitHubIssue, "names with dashes"),
            ("https://github.com/org_name/repo.name/pull/456", UrlType::GitHubIssue, "names with underscores and dots"),
            ("https://github.com/user123/project_name/issues/789", UrlType::GitHubIssue, "names with numbers and underscores"),
            
            // Invalid path structures
            ("https://github.com/issues/123", UrlType::Html, "missing repo segment"),
            ("https://github.com/owner/issues/123", UrlType::Html, "missing repo, direct to issues"),
            ("https://github.com/owner/repo/123", UrlType::Html, "missing issues/pull segment"),
            ("https://github.com//repo/issues/123", UrlType::Html, "empty owner segment"),
            ("https://github.com/owner//issues/123", UrlType::Html, "empty repo segment"),
        ];

        helpers::test_github_url_classification(&test_cases);
    }
}

/// Tests for URL normalization
mod url_normalization_tests {
    use super::*;

    #[test]
    fn test_normalize_tracking_params() {
        let test_cases = [
            (
                "https://example.com/page?utm_source=email&content=important&utm_medium=social",
                "https://example.com/page?content=important",
            ),
            (
                "https://example.com/page?utm_campaign=test&ref=twitter&important=keep",
                "https://example.com/page?important=keep",
            ),
            (
                "https://example.com/page?gclid=123&fbclid=456&content=preserve",
                "https://example.com/page?content=preserve",
            ),
            (
                "https://example.com/page?_ga=123&_gid=456&mc_cid=789&value=keep",
                "https://example.com/page?value=keep",
            ),
        ];

        helpers::assert_normalization(&test_cases);
    }

    #[test]
    fn test_normalize_all_tracking_params() {
        let test_cases = [(
            "https://example.com/page?utm_source=test&utm_medium=email&utm_campaign=launch&utm_term=keyword&utm_content=ad&ref=social&source=newsletter&campaign=promo&medium=banner&term=search&gclid=google&fbclid=facebook&msclkid=bing&_ga=analytics&_gid=analytics2&mc_cid=mailchimp&mc_eid=mailchimp2",
            "https://example.com/page",
        )];

        helpers::assert_normalization(&test_cases);
    }

    #[test]
    fn test_normalize_preserved_params() {
        let test_cases = [
            (
                "https://docs.google.com/document/d/123/edit?usp=sharing&utm_source=email",
                "https://docs.google.com/document/d/123/edit?usp=sharing",
            ),
            (
                "https://example.com/search?q=rust&utm_campaign=test&page=2",
                "https://example.com/search?q=rust&page=2",
            ),
            (
                "https://api.example.com/data?api_key=secret&ref=tracking&format=json",
                "https://api.example.com/data?api_key=secret&format=json",
            ),
        ];

        helpers::assert_normalization(&test_cases);
    }

    #[test]
    fn test_normalize_handles_empty_query_values() {
        let detector = helpers::create_detector();

        let test_cases = [
            (
                "https://example.com/page?flag&utm_source=test",
                "https://example.com/page?flag",
            ),
            (
                "https://example.com/page?empty=&keep=value&utm_medium=email",
                "https://example.com/page?empty=&keep=value",
            ),
        ];

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            helpers::assert_query_params(&result, expected);
        }
    }

    #[test]
    fn test_normalize_whitespace_handling() {
        let test_cases = [
            ("  https://example.com/page  ", "https://example.com/page"),
            (
                "\t\nhttps://example.com/page\t\n",
                "https://example.com/page",
            ),
            (
                "https://example.com/page?param=value  ",
                "https://example.com/page?param=value",
            ),
        ];

        helpers::assert_normalization(&test_cases);
    }

    #[test]
    fn test_normalize_handles_no_query_parameters() {
        let detector = helpers::create_detector();

        let test_cases = [
            ("https://example.com", "https://example.com/"),
            ("https://example.com/", "https://example.com/"),
            ("https://example.com/page", "https://example.com/page"),
            (
                "https://example.com/path/to/resource",
                "https://example.com/path/to/resource",
            ),
        ];

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(
                result, expected,
                "URL normalization failed for: {input} -> {result}, expected: {expected}"
            );
        }
    }

    #[test]
    fn test_normalize_handles_fragment_identifiers() {
        let test_cases = [
            (
                "https://example.com/page?utm_source=test#section",
                "https://example.com/page#section",
            ),
            (
                "https://example.com/page?keep=value&utm_medium=email#heading",
                "https://example.com/page?keep=value#heading",
            ),
        ];

        helpers::assert_normalization(&test_cases);
    }
}

/// Tests for URL validation
mod url_validation_tests {
    use super::*;

    #[test]
    fn test_validate_valid_urls() {
        helpers::assert_urls_valid(VALID_TEST_URLS);
    }

    #[test]
    fn test_validate_invalid_urls() {
        helpers::assert_urls_invalid(INVALID_TEST_URLS);
    }

    #[test]
    fn test_validate_url_with_whitespace() {
        let detector = helpers::create_detector();

        let urls_with_whitespace = [
            "  https://example.com  ",
            "\t\nhttps://example.com\t\n",
            " https://example.com/path ",
        ];

        for url in urls_with_whitespace {
            let result = detector.validate_url(url);
            assert!(
                result.is_ok(),
                "Should validate URL with whitespace: {url:?}"
            );
        }
    }
}

/// Tests for edge cases and error conditions
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_detect_type_with_malformed_urls() {
        // Test clearly invalid URLs that should fail
        let invalid_urls = [
            "not-a-url",
            "ftp://example.com",
            "",
            "   ",
            "example.com",     // Missing protocol
            "www.example.com", // Missing protocol
            "//example.com",   // Missing protocol
        ];

        helpers::test_urls_with(&invalid_urls, |url, result| {
            assert!(result.is_err(), "Should fail for invalid URL: {url}");
        });

        // Test URLs that might be parsed differently by the URL library
        // but should still be handled gracefully
        let potentially_problematic = ["https://", "http://", "https:///path"];

        helpers::test_urls_with(&potentially_problematic, |_url, result| {
            // Don't assert failure - just ensure it doesn't panic
            // Some of these might actually be parsed successfully by the URL library
            match result {
                Ok(_) => {
                    // If it succeeds, that's fine - the URL library is permissive
                }
                Err(_) => {
                    // If it fails, that's also fine - it's a malformed URL
                }
            }
        });
    }

    #[test]
    fn test_normalize_url_with_malformed_urls() {
        let detector = helpers::create_detector();

        let malformed_urls = ["not-a-url", "ftp://example.com", "", "   "];

        for url in malformed_urls {
            let result = detector.normalize_url(url);
            assert!(result.is_err(), "Should fail for malformed URL: {url}");
        }
    }

    #[test]
    fn test_international_domain_names() {
        // These might not work in all environments, but should not panic
        let idn_urls = [
            "https://例え.テスト/path",
            "https://тест.рф/page",
            "https://test.中国/resource",
        ];

        helpers::test_urls_with(&idn_urls, |_url, result| {
            // We don't assert success/failure as IDN support varies,
            // but it should not panic
            match result {
                Ok(url_type) => {
                    // Should default to HTML for unknown domains
                    assert_eq!(url_type, UrlType::Html);
                }
                Err(_) => {
                    // Also acceptable - IDN parsing might fail
                }
            }
        });
    }

    #[test]
    fn test_very_long_urls() {
        let long_path = "a".repeat(TEST_VERY_LONG_PATH_LENGTH);
        let long_urls = [format!("https://example.com/{long_path}")];
        
        helpers::assert_urls_are_html(&long_urls.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        
        // Test normalization works too
        let detector = helpers::create_detector();
        let normalized = detector.normalize_url(&long_urls[0]);
        assert!(normalized.is_ok());
    }

    #[test]
    fn test_urls_with_special_characters() {
        let special_char_urls = [
            "https://example.com/path%20with%20spaces",
            "https://example.com/path?query=value%20with%20spaces",
            "https://example.com/path/with-dashes",
            "https://example.com/path/with_underscores",
            "https://example.com/path/with.dots",
            "https://example.com/path/with+plus",
            "https://example.com/path?query=value&other=test%26encoded",
        ];

        helpers::assert_urls_are_html(&special_char_urls);
    }

    #[test]
    fn test_case_sensitivity() {
        let detector = helpers::create_detector();

        // Domain names should be case-insensitive
        let case_variants = [
            (
                "https://DOCS.GOOGLE.COM/document/d/123/edit",
                UrlType::GoogleDocs,
            ),
            (
                "https://docs.Google.com/document/d/123/edit",
                UrlType::GoogleDocs,
            ),
            (
                "https://GITHUB.COM/owner/repo/issues/123",
                UrlType::GitHubIssue,
            ),
            (
                "https://GitHub.com/owner/repo/pull/456",
                UrlType::GitHubIssue,
            ),
        ];

        for (url, expected_type) in case_variants {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(
                result, expected_type,
                "Case sensitivity test failed for: {url}"
            );
        }
    }
}

/// Tests for wildcard domain matching
mod wildcard_domain_tests {
    use super::*;

    #[test]
    fn test_non_matching_domains() {
        // These should NOT match the wildcard patterns
        let non_matching_domains = [
            "https://notsharepoint.com/sites/team", // Doesn't end with .sharepoint.com
            "https://fakesharepoint.com/sites/team", // Doesn't end with .sharepoint.com
            "https://example.com/sharepoint",       // Contains sharepoint but wrong domain
            "https://office.example.com/document",  // Contains office but wrong domain
            "https://outlook.example.com/mail",     // Contains outlook but wrong domain
        ];

        helpers::assert_urls_are_html(&non_matching_domains);
    }
}

/// Property-based tests for robustness
mod property_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_all_methods_never_panic(url in ".*") {
            let detector = helpers::create_detector();
            let _ = detector.detect_type(&url);
            let _ = detector.normalize_url(&url);
            let _ = detector.validate_url(&url);
        }
    }

        #[test]
        fn test_valid_http_urls_detected(
            domain in format!(r"[a-zA-Z0-9][a-zA-Z0-9\-]{{0,{}}}[a-zA-Z0-9]", MAX_DOMAIN_LABEL_LENGTH).as_str(),
            tld in format!(r"[a-zA-Z]{{{},{}}}",  MIN_TLD_LENGTH, MAX_TLD_LENGTH).as_str(),
            path in r"/[a-zA-Z0-9\-._~!$&'()*+,;=:@]*"
        ) {
            let url = format!("https://{domain}.{tld}{path}");
            let detector = helpers::create_detector();

            let result = detector.detect_type(&url);
            // Should successfully detect a valid type for well-formed URLs
            // We verify detection succeeded without enumerating specific types
            // so this test remains valid when new URL types are added
            assert!(result.is_ok(), "Should successfully detect type for valid URL: {}", url);
        }

        #[test]
        fn test_normalization_preserves_scheme_and_host(
            scheme in r"https?",
            host in format!(r"[a-zA-Z0-9][a-zA-Z0-9\-]{{0,{}}}[a-zA-Z0-9]\.[a-zA-Z]{{{},{}}}", MAX_DOMAIN_LABEL_LENGTH, MIN_TLD_LENGTH, MAX_TLD_LENGTH).as_str()
        ) {
            let url = format!("{scheme}://{host}");
            let detector = helpers::create_detector();

            if let Ok(normalized) = detector.normalize_url(&url) {
                assert!(normalized.starts_with(&format!("{scheme}://")));
                // Host might be normalized to lowercase, so check case-insensitively
                let normalized_lower = normalized.to_lowercase();
                let host_lower = host.to_lowercase();
                assert!(normalized_lower.contains(&host_lower));
            }
        }
    }
}

/// Integration tests combining detection and normalization
mod integration_tests {
    use super::*;

    #[test]
    fn test_detect_then_normalize() {
        let detector = helpers::create_detector();

        let test_cases = [
            (
                "https://docs.google.com/document/d/123/edit?utm_source=email&usp=sharing",
                UrlType::GoogleDocs,
                "https://docs.google.com/document/d/123/edit?usp=sharing",
            ),
            (
                "https://github.com/owner/repo/issues/123?ref=notification&utm_campaign=test",
                UrlType::GitHubIssue,
                "https://github.com/owner/repo/issues/123",
            ),
        ];

        for (original_url, expected_type, expected_normalized) in test_cases {
            // First detect the type
            let detected_type = detector.detect_type(original_url).unwrap();
            assert_eq!(detected_type, expected_type);

            // Then normalize
            let normalized = detector.normalize_url(original_url).unwrap();
            assert_eq!(normalized, expected_normalized);

            // Detection should still work on normalized URL
            let detected_type_after_normalize = detector.detect_type(&normalized).unwrap();
            assert_eq!(detected_type_after_normalize, expected_type);
        }
    }

    #[test]
    fn test_normalize_then_detect() {
        let detector = helpers::create_detector();

        let urls_with_tracking = [
            "https://docs.google.com/document/d/123/edit?utm_source=email&usp=sharing&utm_medium=social",
            "https://github.com/owner/repo/issues/123?ref=notification&utm_campaign=test&tab=timeline",
            "https://example.com/article?utm_source=newsletter&category=tech&utm_campaign=weekly",
        ];

        for url in urls_with_tracking {
            // First normalize
            let normalized = detector.normalize_url(url).unwrap();

            // Then detect type on normalized URL
            let original_type = detector.detect_type(url).unwrap();
            let normalized_type = detector.detect_type(&normalized).unwrap();

            // Type should be the same
            assert_eq!(
                original_type, normalized_type,
                "Type changed after normalization for: {url}"
            );

            // Normalized URL should not contain tracking parameters
            assert!(!normalized.contains("utm_"));
            assert!(!normalized.contains("ref="));
        }
    }

    #[test]
    fn test_roundtrip_stability() {
        let detector = helpers::create_detector();

        let test_urls = [
            "https://docs.google.com/document/d/123/edit",
            "https://github.com/owner/repo/issues/123",
            "https://company.sharepoint.com/sites/team",
            "https://example.com/article",
        ];

        for url in test_urls {
            helpers::assert_roundtrip_stable(url, &detector);
        }
    }
}
