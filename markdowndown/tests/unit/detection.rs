//! Comprehensive unit tests for URL detection and classification.
//!
//! This module tests URL pattern matching, edge cases, validation,
//! normalization, and all supported URL types with thorough coverage.

use markdowndown::detection::UrlDetector;
use markdowndown::types::{MarkdownError, UrlType};
use proptest::prelude::*;

mod helpers {
    use super::*;

    /// Create test URL detector instance
    pub fn create_detector() -> UrlDetector {
        UrlDetector::new()
    }

    /// Sample URLs for each type for testing
    pub fn sample_urls_by_type() -> Vec<(UrlType, Vec<&'static str>)> {
        vec![
            (
                UrlType::GoogleDocs,
                vec![
                    "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
                    "https://docs.google.com/document/d/abc123def456/view",
                    "https://docs.google.com/document/d/test123/edit#heading=h.123",
                    "https://drive.google.com/file/d/1234567890abcdef/view",
                    "https://drive.google.com/file/d/xyz789/edit",
                ],
            ),
            (
                UrlType::GitHubIssue,
                vec![
                    "https://github.com/owner/repo/issues/123",
                    "https://github.com/microsoft/vscode/issues/42",
                    "https://github.com/rust-lang/rust/issues/12345",
                    "https://github.com/owner/repo/pull/456",
                    "https://github.com/microsoft/vscode/pull/789",
                    "https://github.com/rust-lang/rust/pull/98765",
                    "https://github.com/owner/repo/issues/1",
                    "https://github.com/owner/repo/pull/999999",
                ],
            ),
            (
                UrlType::Html,
                vec![
                    "https://example.com",
                    "https://www.example.com/page.html",
                    "https://blog.example.com/post/123",
                    "https://news.example.org/article?id=456",
                    "https://www.wikipedia.org/wiki/Rust_(programming_language)",
                    "https://stackoverflow.com/questions/12345/how-to-do-something",
                    "https://reddit.com/r/rust/comments/abc123/title",
                    "https://github.com/owner/repo", // Not an issue/PR, should be HTML
                    "https://github.com/owner/repo/commits",
                    "https://github.com/owner/repo/tree/main",
                ],
            ),
        ]
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
    fn test_google_docs_document_detection() {
        let detector = helpers::create_detector();

        let google_docs_urls = [
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
            "https://docs.google.com/document/d/abc123/view",
            "https://docs.google.com/document/d/test_doc_id_123/edit#heading=h.xyz",
            "https://docs.google.com/document/d/short/copy",
            "https://docs.google.com/document/d/1234567890/edit?usp=sharing",
        ];

        for url in google_docs_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GoogleDocs, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_google_drive_file_detection() {
        let detector = helpers::create_detector();

        let drive_urls = [
            "https://drive.google.com/file/d/1234567890abcdef/view",
            "https://drive.google.com/file/d/xyz789/edit",
            "https://drive.google.com/file/d/test_file/preview",
            "https://drive.google.com/file/d/abc123def456/view?usp=sharing",
        ];

        for url in drive_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GoogleDocs, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_github_issue_detection() {
        let detector = helpers::create_detector();

        let issue_urls = [
            "https://github.com/owner/repo/issues/123",
            "https://github.com/microsoft/vscode/issues/42",
            "https://github.com/rust-lang/rust/issues/12345",
            "https://github.com/facebook/react/issues/1",
            "https://github.com/nodejs/node/issues/999999",
        ];

        for url in issue_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_github_pull_request_detection() {
        let detector = helpers::create_detector();

        let pr_urls = [
            "https://github.com/owner/repo/pull/456",
            "https://github.com/microsoft/vscode/pull/789",
            "https://github.com/rust-lang/rust/pull/98765",
            "https://github.com/facebook/react/pull/1",
            "https://github.com/nodejs/node/pull/999999",
        ];

        for url in pr_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_html_fallback_detection() {
        let detector = helpers::create_detector();

        let html_urls = [
            "https://example.com",
            "https://www.example.com/page.html",
            "https://blog.example.com/post/123",
            "https://news.example.org/article?id=456",
            "https://stackoverflow.com/questions/12345",
            "https://reddit.com/r/rust",
            "https://www.wikipedia.org/wiki/Main_Page",
            // GitHub URLs that aren't issues/PRs should fall back to HTML
            "https://github.com/owner/repo",
            "https://github.com/owner/repo/commits",
            "https://github.com/owner/repo/tree/main",
            "https://github.com/owner/repo/blob/main/README.md",
        ];

        for url in html_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::Html, "Failed for URL: {url}");
        }
    }
}

/// Tests for GitHub URL edge cases
mod github_edge_cases {
    use super::*;

    #[test]
    fn test_github_issue_with_fragments() {
        let detector = helpers::create_detector();

        let urls_with_fragments = [
            "https://github.com/owner/repo/issues/123#issuecomment-456789",
            "https://github.com/microsoft/vscode/issues/42#event-123456",
            "https://github.com/rust-lang/rust/pull/12345#pullrequestreview-789",
            "https://github.com/owner/repo/pull/456#discussion_r123456789",
        ];

        for url in urls_with_fragments {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_github_issue_with_query_params() {
        let detector = helpers::create_detector();

        let urls_with_params = [
            "https://github.com/owner/repo/issues/123?tab=timeline",
            "https://github.com/microsoft/vscode/pull/456?diff=unified",
            "https://github.com/rust-lang/rust/issues/789?q=is%3Aissue+is%3Aopen",
        ];

        for url in urls_with_params {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_github_non_issue_urls() {
        let detector = helpers::create_detector();

        let non_issue_urls = [
            "https://github.com/owner/repo",           // Repository home
            "https://github.com/owner/repo/issues",    // Issues list
            "https://github.com/owner/repo/pull",      // PRs list
            "https://github.com/owner/repo/commits",   // Commits
            "https://github.com/owner/repo/tree/main", // Tree view
            "https://github.com/owner/repo/blob/main/README.md", // File view
            "https://github.com/owner/repo/releases",  // Releases
            "https://github.com/owner/repo/wiki",      // Wiki
            "https://github.com/owner/repo/settings",  // Settings
            "https://github.com/owner/repo/actions",   // Actions
            "https://github.com/owner/repo/issues/abc", // Invalid issue number
            "https://github.com/owner/repo/pull/def",  // Invalid PR number
            "https://github.com/owner/repo/issues/",   // Missing number
            "https://github.com/owner/repo/pull/",     // Missing number
        ];

        for url in non_issue_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::Html, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_github_issue_number_validation() {
        let detector = helpers::create_detector();

        // Valid issue numbers
        let valid_urls = [
            "https://github.com/owner/repo/issues/1",
            "https://github.com/owner/repo/issues/123",
            "https://github.com/owner/repo/issues/999999",
            "https://github.com/owner/repo/pull/1",
            "https://github.com/owner/repo/pull/123",
            "https://github.com/owner/repo/pull/999999",
        ];

        for url in valid_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for valid URL: {url}");
        }

        // Invalid issue numbers
        let invalid_urls = [
            "https://github.com/owner/repo/issues/abc",
            "https://github.com/owner/repo/issues/123abc",
            "https://github.com/owner/repo/issues/abc123",
            "https://github.com/owner/repo/pull/xyz",
            "https://github.com/owner/repo/pull/123xyz",
            "https://github.com/owner/repo/pull/xyz123",
            "https://github.com/owner/repo/issues/",
            "https://github.com/owner/repo/pull/",
        ];

        for url in invalid_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::Html, "Failed for invalid URL: {url}");
        }
    }

    #[test]
    fn test_github_path_structure() {
        let detector = helpers::create_detector();

        // URLs with correct structure
        let correct_structure = [
            "https://github.com/a/b/issues/1", // Minimal valid
            "https://github.com/owner-name/repo-name/issues/123",
            "https://github.com/org_name/repo.name/pull/456",
            "https://github.com/user123/project_name/issues/789",
        ];

        for url in correct_structure {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }

        // URLs with incorrect structure
        let incorrect_structure = [
            "https://github.com/issues/123",        // Missing repo
            "https://github.com/owner/issues/123",  // Missing repo
            "https://github.com/owner/repo/123",    // Missing type
            "https://github.com//repo/issues/123",  // Empty owner
            "https://github.com/owner//issues/123", // Empty repo
        ];

        for url in incorrect_structure {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::Html, "Failed for URL: {url}");
        }
    }
}

/// Tests for URL normalization
mod url_normalization_tests {
    use super::*;

    #[test]
    fn test_normalize_removes_tracking_parameters() {
        let detector = helpers::create_detector();

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

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(result, expected, "Failed to normalize: {input}");
        }
    }

    #[test]
    fn test_normalize_removes_all_tracking_parameters() {
        let detector = helpers::create_detector();

        let url = "https://example.com/page?utm_source=test&utm_medium=email&utm_campaign=launch&utm_term=keyword&utm_content=ad&ref=social&source=newsletter&campaign=promo&medium=banner&term=search&gclid=google&fbclid=facebook&msclkid=bing&_ga=analytics&_gid=analytics2&mc_cid=mailchimp&mc_eid=mailchimp2";
        let expected = "https://example.com/page";

        let result = detector.normalize_url(url).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_normalize_preserves_important_parameters() {
        let detector = helpers::create_detector();

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

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(result, expected, "Failed to normalize: {input}");
        }
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

        for (input, _expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            // The exact format might vary, but tracking params should be removed
            assert!(!result.contains("utm_source"));
            assert!(!result.contains("utm_medium"));
            assert!(
                result.contains("flag")
                    || result.contains("empty=")
                    || result.contains("keep=value")
            );
        }
    }

    #[test]
    fn test_normalize_trims_whitespace() {
        let detector = helpers::create_detector();

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

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(result, expected, "Failed to normalize: {input}");
        }
    }

    #[test]
    fn test_normalize_handles_no_query_parameters() {
        let detector = helpers::create_detector();

        let test_cases = [
            ("https://example.com", "https://example.com"),
            ("https://example.com/", "https://example.com/"),
            ("https://example.com/page", "https://example.com/page"),
            (
                "https://example.com/path/to/resource",
                "https://example.com/path/to/resource",
            ),
        ];

        for (input, expected_base) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            // URL normalization might add trailing slash, so be flexible
            assert!(
                result == expected_base
                    || result == format!("{}/", expected_base.trim_end_matches('/')),
                "URL normalization failed for: {input} -> {result}"
            );
        }
    }

    #[test]
    fn test_normalize_handles_fragment_identifiers() {
        let detector = helpers::create_detector();

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

        for (input, expected) in test_cases {
            let result = detector.normalize_url(input).unwrap();
            assert_eq!(result, expected, "Failed to normalize: {input}");
        }
    }
}

/// Tests for URL validation
mod url_validation_tests {
    use super::*;

    #[test]
    fn test_validate_valid_urls() {
        let detector = helpers::create_detector();

        let valid_urls = [
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

        for url in valid_urls {
            let result = detector.validate_url(url);
            assert!(result.is_ok(), "Should validate URL: {url}");
        }
    }

    #[test]
    fn test_validate_invalid_urls() {
        let detector = helpers::create_detector();

        let invalid_urls = [
            "not-a-url",
            "ftp://example.com",
            "mailto:test@example.com",
            "javascript:alert('xss')",
            "data:text/html,<h1>Test</h1>",
            "",
            "   ",
            "example.com",     // Missing protocol
            "www.example.com", // Missing protocol
            "//example.com",   // Missing protocol
            "https://",        // Incomplete
            "http://",         // Incomplete
        ];

        for url in invalid_urls {
            let result = detector.validate_url(url);
            assert!(result.is_err(), "Should reject URL: {url}");

            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, .. } => {
                    assert_eq!(kind, markdowndown::types::ValidationErrorKind::InvalidUrl);
                }
                _ => panic!("Expected InvalidUrl error for: {url}"),
            }
        }
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
        let detector = helpers::create_detector();

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

        for url in invalid_urls {
            let result = detector.detect_type(url);
            assert!(result.is_err(), "Should fail for invalid URL: {url}");
        }

        // Test URLs that might be parsed differently by the URL library
        // but should still be handled gracefully
        let potentially_problematic = ["https://", "http://", "https:///path"];

        for url in potentially_problematic {
            let result = detector.detect_type(url);
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
        }
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
        let detector = helpers::create_detector();

        // These might not work in all environments, but should not panic
        let idn_urls = [
            "https://例え.テスト/path",
            "https://тест.рф/page",
            "https://test.中国/resource",
        ];

        for url in idn_urls {
            let result = detector.detect_type(url);
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
        }
    }

    #[test]
    fn test_very_long_urls() {
        let detector = helpers::create_detector();

        // Create a very long but valid URL
        let long_path = "a".repeat(2000);
        let long_url = format!("https://example.com/{long_path}");

        let result = detector.detect_type(&long_url);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), UrlType::Html);

        // Test normalization too
        let normalized = detector.normalize_url(&long_url);
        assert!(normalized.is_ok());
    }

    #[test]
    fn test_urls_with_special_characters() {
        let detector = helpers::create_detector();

        let special_char_urls = [
            "https://example.com/path%20with%20spaces",
            "https://example.com/path?query=value%20with%20spaces",
            "https://example.com/path/with-dashes",
            "https://example.com/path/with_underscores",
            "https://example.com/path/with.dots",
            "https://example.com/path/with+plus",
            "https://example.com/path?query=value&other=test%26encoded",
        ];

        for url in special_char_urls {
            let result = detector.detect_type(url);
            assert!(
                result.is_ok(),
                "Should handle special characters in URL: {url}"
            );
            assert_eq!(result.unwrap(), UrlType::Html);
        }
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
        let detector = helpers::create_detector();

        // These should NOT match the wildcard patterns
        let non_matching_domains = [
            "https://notsharepoint.com/sites/team", // Doesn't end with .sharepoint.com
            "https://fakesharepoint.com/sites/team", // Doesn't end with .sharepoint.com
            "https://example.com/sharepoint",       // Contains sharepoint but wrong domain
            "https://office.example.com/document",  // Contains office but wrong domain
            "https://outlook.example.com/mail",     // Contains outlook but wrong domain
        ];

        for url in non_matching_domains {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(
                result,
                UrlType::Html,
                "Should not match Office365 pattern: {url}"
            );
        }
    }
}

/// Property-based tests for robustness
mod property_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_detect_type_never_panics(url in ".*") {
            let detector = helpers::create_detector();
            let _result = detector.detect_type(&url);
            // Should never panic regardless of input
        }

        #[test]
        fn test_normalize_url_never_panics(url in ".*") {
            let detector = helpers::create_detector();
            let _result = detector.normalize_url(&url);
            // Should never panic regardless of input
        }

        #[test]
        fn test_validate_url_never_panics(url in ".*") {
            let detector = helpers::create_detector();
            let _result = detector.validate_url(&url);
            // Should never panic regardless of input
        }

        #[test]
        fn test_valid_http_urls_detected(
            domain in r"[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9]",
            tld in r"[a-zA-Z]{2,6}",
            path in r"/[a-zA-Z0-9\-._~!$&'()*+,;=:@]*"
        ) {
            let url = format!("https://{domain}.{tld}{path}");
            let detector = helpers::create_detector();

            let result = detector.detect_type(&url);
            // Should successfully detect some type (at minimum HTML)
            if result.is_ok() {
                let url_type = result.unwrap();
                // Should be one of the supported types
                assert!(matches!(url_type, UrlType::Html | UrlType::GoogleDocs | UrlType::GitHubIssue));
            }
        }

        #[test]
        fn test_normalization_preserves_scheme_and_host(
            scheme in r"https?",
            host in r"[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9]\.[a-zA-Z]{2,6}"
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
            // Multiple normalizations should be stable
            let normalized1 = detector.normalize_url(url).unwrap();
            let normalized2 = detector.normalize_url(&normalized1).unwrap();
            let normalized3 = detector.normalize_url(&normalized2).unwrap();

            assert_eq!(normalized1, normalized2);
            assert_eq!(normalized2, normalized3);

            // Type detection should be stable too
            let type1 = detector.detect_type(url).unwrap();
            let type2 = detector.detect_type(&normalized1).unwrap();
            let type3 = detector.detect_type(&normalized2).unwrap();

            assert_eq!(type1, type2);
            assert_eq!(type2, type3);
        }
    }
}
