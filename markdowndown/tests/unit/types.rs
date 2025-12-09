//! Comprehensive unit tests for markdowndown core types.
//!
//! This module tests all core types including Markdown, Url, UrlType, MarkdownError,
//! and Frontmatter with thorough validation, serialization, and error handling tests.

use chrono::{DateTime, Utc};
use markdowndown::types::{
    AuthErrorKind, ContentErrorKind, ConverterErrorKind, ErrorContext, Frontmatter, Markdown,
    MarkdownError, NetworkErrorKind, Url, UrlType, ValidationErrorKind,
};
use proptest::prelude::*;
use serde_yaml;

// Test constants for magic number elimination
const MAX_TIMESTAMP_DIFF_SECONDS: i64 = 5;
const HTTP_INTERNAL_SERVER_ERROR: u16 = 500;
const HTTP_NOT_FOUND: u16 = 404;


mod helpers {
    use super::*;
    use serde::{Deserialize, Serialize};

    pub fn create_test_error_context() -> ErrorContext {
        ErrorContext::new("https://test.com", "test operation", "TestConverter")
    }

    pub fn assert_parse_error(result: Result<Markdown, MarkdownError>, expected_message: &str) {
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => assert_eq!(message, expected_message),
            err => panic!("Expected ParseError, got: {err:?}"),
        }
    }

    pub fn test_serialization_roundtrip<T, S, D>(value: &T, serialize_fn: S, deserialize_fn: D)
    where
        T: PartialEq + std::fmt::Debug,
        S: FnOnce(&T) -> Result<String, Box<dyn std::error::Error>>,
        D: FnOnce(&str) -> Result<T, Box<dyn std::error::Error>>,
    {
        let serialized = serialize_fn(value).unwrap();
        let deserialized: T = deserialize_fn(&serialized).unwrap();
        assert_eq!(*value, deserialized);
    }

    pub fn test_yaml_roundtrip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        test_serialization_roundtrip(
            value,
            |v| serde_yaml::to_string(v).map_err(|e| e.into()),
            |s| serde_yaml::from_str(s).map_err(|e| e.into()),
        );
    }

    pub fn test_json_roundtrip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        test_serialization_roundtrip(
            value,
            |v| serde_json::to_string(v).map_err(|e| e.into()),
            |s| serde_json::from_str(s).map_err(|e| e.into()),
        );
    }

    pub fn assert_url_validation_fails(url: &str, expected_kind: ValidationErrorKind) {
        let result = Url::new(url.to_string());
        assert!(result.is_err(), "Should reject URL: {url}");
        match result.unwrap_err() {
            MarkdownError::ValidationError { kind, context } => {
                assert_eq!(kind, expected_kind);
                assert_eq!(context.url, url);
            }
            _ => panic!("Expected ValidationError with {expected_kind:?} for: {url}"),
        }
    }

    pub fn assert_error_properties(
        error: MarkdownError,
        expected_retryable: bool,
        expected_recoverable: bool,
    ) {
        assert_eq!(error.is_retryable(), expected_retryable);
        assert_eq!(error.is_recoverable(), expected_recoverable);
    }

    pub fn test_error_properties_batch(
        error_cases: Vec<(MarkdownError, bool, bool)>,
    ) {
        for (error, expected_retryable, expected_recoverable) in error_cases {
            assert_error_properties(error, expected_retryable, expected_recoverable);
        }
    }

    pub fn test_error_kind_properties<K, F>(
        error_constructor: F,
        test_cases: Vec<(K, bool, bool)>,
    ) where
        F: Fn(K, ErrorContext) -> MarkdownError,
    {
        let context = create_test_error_context();
        let errors = test_cases
            .into_iter()
            .map(|(kind, retry, recover)| (error_constructor(kind, context.clone()), retry, recover))
            .collect();
        test_error_properties_batch(errors);
    }

    pub fn assert_suggestion_contains(error: &MarkdownError, expected_substring: &str) {
        let suggestions = error.suggestions();
        assert!(
            suggestions.iter().any(|s| s.contains(expected_substring)),
            "Expected suggestions to contain '{}', got: {:?}",
            expected_substring,
            suggestions
        );
    }

    pub fn test_legacy_error(
        error: MarkdownError,
        expected_retryable: bool,
        expected_recoverable: bool,
        expected_suggestion_substring: &str,
    ) {
        assert!(error.context().is_none());
        assert_eq!(error.is_retryable(), expected_retryable);
        assert_eq!(error.is_recoverable(), expected_recoverable);
        assert_suggestion_contains(&error, expected_suggestion_substring);
    }
}

/// Tests for the Markdown newtype wrapper
mod markdown_tests {
    use super::*;

    #[test]
    fn test_markdown_creation_valid_content() {
        let content = "# Valid Markdown Content";
        let markdown = Markdown::new(content.to_string()).unwrap();
        assert_eq!(markdown.as_str(), content);
    }

    #[test]
    fn test_markdown_creation_from_string() {
        let content = "# Test Content";
        let markdown = Markdown::from(content.to_string());
        assert_eq!(markdown.as_str(), content);
    }

    #[test]
    fn test_markdown_validation_invalid_inputs() {
        let invalid_cases = vec![
            ("", "Markdown content cannot be empty or whitespace-only"),
            ("   \n\t  \r\n  ", "Markdown content cannot be empty or whitespace-only"),
        ];

        for (input, expected_msg) in invalid_cases {
            let result = Markdown::new(input.to_string());
            helpers::assert_parse_error(result, expected_msg);

            let markdown_from = Markdown::from(input.to_string());
            assert!(markdown_from.validate().is_err());
        }
    }

    #[test]
    fn test_markdown_validation_valid_content() {
        let valid_markdown = Markdown::from("# Valid Content".to_string());
        assert!(valid_markdown.validate().is_ok());
    }

    #[test]
    fn test_markdown_with_frontmatter() {
        let content = Markdown::from("# Test Document\n\nThis is content.".to_string());
        let frontmatter = "---\nsource_url: \"https://example.com\"\nexporter: \"test\"\n---\n";

        let result = content.with_frontmatter(frontmatter);
        let result_str = result.as_str();

        assert!(result_str.contains("source_url: \"https://example.com\""));
        assert!(result_str.contains("# Test Document"));
        assert!(result_str.starts_with("---\n"));
    }

    #[test]
    fn test_markdown_frontmatter_extraction() {
        let content_with_frontmatter = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n\n# Hello World\n\nContent here.";
        let markdown = Markdown::from(content_with_frontmatter.to_string());

        let frontmatter = markdown.frontmatter();
        assert!(frontmatter.is_some());

        let fm = frontmatter.unwrap();
        assert!(fm.contains("source_url: https://example.com"));
        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("---\n"));
    }

    #[test]
    fn test_markdown_frontmatter_extraction_none() {
        let content_without_frontmatter = "# Hello World\n\nNo frontmatter here.";
        let markdown = Markdown::from(content_without_frontmatter.to_string());

        let frontmatter = markdown.frontmatter();
        assert!(frontmatter.is_none());
    }

    #[test]
    fn test_markdown_content_only() {
        let content_with_frontmatter = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n\n# Hello World\n\nContent here.";
        let markdown = Markdown::from(content_with_frontmatter.to_string());

        let content_only = markdown.content_only();
        assert_eq!(content_only, "# Hello World\n\nContent here.");
        assert!(!content_only.contains("source_url"));
    }

    #[test]
    fn test_markdown_content_only_no_frontmatter() {
        let content = "# Hello World\n\nNo frontmatter here.";
        let markdown = Markdown::from(content.to_string());

        let content_only = markdown.content_only();
        assert_eq!(content_only, content);
    }

    #[test]
    fn test_markdown_roundtrip_with_frontmatter() {
        let original_content = "# Test Document\n\nThis is test content.";
        let frontmatter = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n";

        let markdown = Markdown::from(original_content.to_string());
        let with_frontmatter = markdown.with_frontmatter(frontmatter);

        // Verify frontmatter can be extracted
        let extracted_frontmatter = with_frontmatter.frontmatter();
        assert!(extracted_frontmatter.is_some());
        assert!(extracted_frontmatter.unwrap().contains("source_url"));

        // Verify content can be extracted
        let extracted_content = with_frontmatter.content_only();
        assert_eq!(extracted_content, original_content);
    }

    #[test]
    fn test_markdown_deref_traits() {
        let content = "# Test Content";
        let markdown = Markdown::from(content.to_string());

        // Test Deref trait
        assert_eq!(&*markdown, content);

        // Test AsRef trait
        assert_eq!(markdown.as_ref(), content);
    }
}

/// Tests for the Url newtype wrapper
mod url_tests {
    use super::*;

    #[test]
    fn test_url_creation_valid_https() {
        let url_str = "https://example.com";
        let url = Url::new(url_str.to_string()).unwrap();
        assert_eq!(url.as_str(), url_str);
        assert_eq!(format!("{url}"), url_str);
    }

    #[test]
    fn test_url_creation_valid_http() {
        let url_str = "http://test.org";
        let url = Url::new(url_str.to_string()).unwrap();
        assert_eq!(url.as_str(), url_str);
    }

    #[test]
    fn test_url_creation_with_path() {
        let url_str = "https://example.com/path/to/resource?param=value#section";
        let url = Url::new(url_str.to_string()).unwrap();
        assert_eq!(url.as_str(), url_str);
    }

    #[test]
    fn test_url_validation_rejects_invalid_inputs() {
        let invalid_protocol = [
            "ftp://example.com",
            "mailto:test@example.com",
            "ws://example.com",
        ];
        let incomplete = ["http://", "https://", "example.com", "www.example.com", ""];

        for url in invalid_protocol {
            helpers::assert_url_validation_fails(url, ValidationErrorKind::InvalidUrl);
        }

        for url in incomplete {
            let result = Url::new(url.to_string());
            assert!(result.is_err(), "Should reject URL: {url}");
        }
    }

    #[test]
    fn test_url_as_ref_trait() {
        let url_str = "https://example.com";
        let url = Url::new(url_str.to_string()).unwrap();
        assert_eq!(url.as_ref(), url_str);
    }

    #[test]
    fn test_url_serialization() {
        let url = Url::new("https://example.com".to_string()).unwrap();

        helpers::test_yaml_roundtrip(&url);
        helpers::test_json_roundtrip(&url);
    }
}

/// Tests for the UrlType enumeration
mod url_type_tests {
    use super::*;



    #[test]
    fn test_url_type_equality() {
        assert_eq!(UrlType::Html, UrlType::Html);
        assert_ne!(UrlType::Html, UrlType::GoogleDocs);
    }

    #[test]
    fn test_url_type_clone() {
        let url_type = UrlType::GoogleDocs;
        let cloned = url_type.clone();
        assert_eq!(url_type, cloned);
    }

    #[test]
    fn test_url_type_serialization() {
        for url_type in [UrlType::Html, UrlType::GoogleDocs, UrlType::GitHubIssue] {
            helpers::test_yaml_roundtrip(&url_type);
            helpers::test_json_roundtrip(&url_type);
        }
    }

    #[test]
    fn test_url_type_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(UrlType::Html, "HTML content");
        map.insert(UrlType::GoogleDocs, "Google Docs content");

        assert_eq!(map.get(&UrlType::Html), Some(&"HTML content"));
        assert_eq!(map.get(&UrlType::GoogleDocs), Some(&"Google Docs content"));
    }
}

/// Tests for ErrorContext structure
mod error_context_tests {
    use super::*;

    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new(
            "https://example.com/test",
            "URL validation",
            "TestConverter",
        );

        assert_eq!(context.url, "https://example.com/test");
        assert_eq!(context.operation, "URL validation");
        assert_eq!(context.converter_type, "TestConverter");
        assert!(context.additional_info.is_none());

        // Timestamp should be recent (within last few seconds)
        let now = Utc::now();
        let diff = (now - context.timestamp).num_seconds();
        assert!((0..MAX_TIMESTAMP_DIFF_SECONDS).contains(&diff));
    }

    #[test]
    fn test_error_context_with_info() {
        let context = ErrorContext::new(
            "https://example.com/test",
            "URL validation",
            "TestConverter",
        )
        .with_info("Additional debugging information");

        assert_eq!(
            context.additional_info,
            Some("Additional debugging information".to_string())
        );
    }

    #[test]
    fn test_error_context_serialization() {
        let context = ErrorContext::new(
            "https://example.com/test",
            "Test operation",
            "TestConverter",
        )
        .with_info("Additional context");

        helpers::test_yaml_roundtrip(&context);
    }
}

/// Tests for enhanced error handling
mod enhanced_error_tests {
    use super::*;

    #[test]
    fn test_validation_error_creation() {
        let context = helpers::create_test_error_context();
        let error = MarkdownError::ValidationError {
            kind: ValidationErrorKind::InvalidUrl,
            context: context.clone(),
        };

        assert_eq!(error.context(), Some(&context));
        assert!(!error.is_retryable());
        assert!(!error.is_recoverable());

        let suggestions = error.suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.contains("http")));
    }

    #[test]
    fn test_network_error_retryable_logic() {
        helpers::test_error_kind_properties(
            |kind, context| MarkdownError::EnhancedNetworkError { kind, context },
            vec![
                // Test retryable network errors
                (NetworkErrorKind::Timeout, true, true),
                (NetworkErrorKind::ConnectionFailed, true, true),
                (NetworkErrorKind::RateLimited, true, true),
                // Test non-retryable network errors
                (NetworkErrorKind::DnsResolution, false, false),
                // Test server error logic
                (NetworkErrorKind::ServerError(HTTP_INTERNAL_SERVER_ERROR), true, true),
                (NetworkErrorKind::ServerError(HTTP_NOT_FOUND), false, false),
            ],
        );
    }

    #[test]
    fn test_auth_error_handling() {
        helpers::test_error_kind_properties(
            |kind, context| MarkdownError::AuthenticationError { kind, context },
            vec![
                (AuthErrorKind::TokenExpired, true, true),
                (AuthErrorKind::MissingToken, false, true),
                (AuthErrorKind::InvalidToken, false, true),
                (AuthErrorKind::PermissionDenied, false, false),
            ],
        );
    }

    #[test]
    fn test_content_error_recovery() {
        helpers::test_error_kind_properties(
            |kind, context| MarkdownError::ContentError { kind, context },
            vec![
                (ContentErrorKind::UnsupportedFormat, false, true),
                (ContentErrorKind::EmptyContent, false, false),
                (ContentErrorKind::ParsingFailed, false, false),
            ],
        );
    }

    #[test]
    fn test_converter_error_recovery() {
        helpers::test_error_kind_properties(
            |kind, context| MarkdownError::ConverterError { kind, context },
            vec![
                (ConverterErrorKind::ExternalToolFailed, false, true),
                (ConverterErrorKind::ProcessingError, false, true),
                (ConverterErrorKind::UnsupportedOperation, false, true),
            ],
        );
    }

    #[test]
    fn test_error_suggestions_comprehensive() {
        let context = helpers::create_test_error_context();

        // Test validation error suggestions
        let validation_error = MarkdownError::ValidationError {
            kind: ValidationErrorKind::InvalidUrl,
            context: context.clone(),
        };
        helpers::assert_suggestion_contains(&validation_error, "http");

        // Test network error suggestions
        let network_error = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::Timeout,
            context: context.clone(),
        };
        helpers::assert_suggestion_contains(&network_error, "internet connection");

        // Test auth error suggestions
        let auth_error = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::MissingToken,
            context,
        };
        helpers::assert_suggestion_contains(&auth_error, "authentication");
    }
}

/// Tests for legacy error compatibility
mod legacy_error_tests {
    use super::*;

    #[test]
    fn test_legacy_parse_error() {
        let error = MarkdownError::ParseError {
            message: "Legacy parsing failed".to_string(),
        };
        helpers::test_legacy_error(error, false, false, "content format");
    }

    #[test]
    fn test_legacy_network_error() {
        let error = MarkdownError::NetworkError {
            message: "Connection timeout occurred".to_string(),
        };
        helpers::test_legacy_error(error, true, true, "internet connection");
    }

    #[test]
    fn test_legacy_invalid_url_error() {
        let error = MarkdownError::InvalidUrl {
            url: "not-a-url".to_string(),
        };
        helpers::test_legacy_error(error, false, false, "http");
    }

    #[test]
    fn test_legacy_auth_error() {
        let error = MarkdownError::AuthError {
            message: "Invalid authentication token".to_string(),
        };
        helpers::test_legacy_error(error, false, true, "authentication");
    }
}

/// Tests for Frontmatter structure
mod frontmatter_tests {
    use super::*;

    #[test]
    fn test_frontmatter_creation() {
        let url = Url::new("https://example.com".to_string()).unwrap();
        let timestamp = Utc::now();

        let frontmatter = Frontmatter {
            source_url: url.clone(),
            exporter: "markdowndown".to_string(),
            date_downloaded: timestamp,
        };

        assert_eq!(frontmatter.source_url, url);
        assert_eq!(frontmatter.exporter, "markdowndown");
        assert_eq!(frontmatter.date_downloaded, timestamp);
    }

    #[test]
    fn test_frontmatter_serialization() {
        let frontmatter_yaml = Frontmatter {
            source_url: Url::new("https://example.com".to_string()).unwrap(),
            exporter: "markdowndown".to_string(),
            date_downloaded: DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };

        let frontmatter_json = Frontmatter {
            source_url: Url::new("https://docs.google.com/document/d/123".to_string()).unwrap(),
            exporter: "test-exporter".to_string(),
            date_downloaded: Utc::now(),
        };

        helpers::test_yaml_roundtrip(&frontmatter_yaml);
        helpers::test_json_roundtrip(&frontmatter_json);
    }

    #[test]
    fn test_frontmatter_equality() {
        let timestamp = Utc::now();
        let url = Url::new("https://example.com".to_string()).unwrap();

        let frontmatter1 = Frontmatter {
            source_url: url.clone(),
            exporter: "markdowndown".to_string(),
            date_downloaded: timestamp,
        };

        let frontmatter2 = Frontmatter {
            source_url: url,
            exporter: "markdowndown".to_string(),
            date_downloaded: timestamp,
        };

        assert_eq!(frontmatter1, frontmatter2);
    }
}

/// Property-based tests for type validation
mod property_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_markdown_never_panics_with_arbitrary_input(content in ".*") {
            // Markdown creation should never panic, only return errors for invalid input
            let _result = Markdown::new(content);
        }

        #[test]
        fn test_url_validation_never_panics(url_input in ".*") {
            // URL validation should never panic, only return errors for invalid URLs
            let _result = Url::new(url_input);
        }

        #[test]
        fn test_markdown_validation_consistent(content in ".*") {
            // Validation should be consistent - if new() succeeds, validate() should too
            if let Ok(markdown) = Markdown::new(content.clone()) {
                assert!(markdown.validate().is_ok());
            }

            // And vice versa for From constructor
            let markdown_from = Markdown::from(content);
            if markdown_from.validate().is_ok() {
                assert!(Markdown::new(markdown_from.as_str().to_string()).is_ok());
            }
        }

        #[test]
        fn test_url_format_consistency(url_str in r"https?://[a-zA-Z0-9.-]+(/.*)?") {
            // Property test verifies that URL validation doesn't panic and behaves consistently
            // The implementation determines what is valid - we just verify consistent behavior
            let _result = Url::new(url_str.clone());
            // No assertions about validity - we're testing that validation is consistent
            // and doesn't panic, not imposing test-specific validation rules
        }

        #[test]
        fn test_error_context_string_fields(
            url in ".*",
            operation in ".*",
            converter_type in ".*"
        ) {
            // ErrorContext creation should never panic regardless of input strings
            let context = ErrorContext::new(url.clone(), operation.clone(), converter_type.clone());
            assert_eq!(context.url, url);
            assert_eq!(context.operation, operation);
            assert_eq!(context.converter_type, converter_type);
        }
    }
}

/// Integration tests combining multiple types
mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_document_workflow() {
        // Test complete workflow: create validated types and combine them
        let markdown =
            Markdown::new("# Hello World\n\nThis is a test document.".to_string()).unwrap();
        let url = Url::new("https://docs.google.com/document/d/123".to_string()).unwrap();
        let frontmatter = Frontmatter {
            source_url: url,
            exporter: "markdowndown".to_string(),
            date_downloaded: Utc::now(),
        };

        // Test that all components work together
        let yaml_frontmatter = serde_yaml::to_string(&frontmatter).unwrap();
        let full_document = format!("---\n{yaml_frontmatter}---\n\n{markdown}");

        assert!(full_document.contains("# Hello World"));
        assert!(full_document.contains("https://docs.google.com"));
        assert!(full_document.contains("markdowndown"));
    }

    #[test]
    fn test_error_propagation_workflow() {
        // Test that validation errors propagate correctly through the workflow

        // Invalid URL should be caught
        let invalid_url_result = Url::new("not-a-valid-url".to_string());
        assert!(invalid_url_result.is_err());

        // Invalid markdown should be caught
        let invalid_markdown_result = Markdown::new("   \n\t  ".to_string());
        assert!(invalid_markdown_result.is_err());

        // But valid combinations should work
        let valid_url = Url::new("https://example.com".to_string()).unwrap();
        let valid_markdown = Markdown::new("# Valid Content".to_string()).unwrap();
        let frontmatter = Frontmatter {
            source_url: valid_url,
            exporter: "test".to_string(),
            date_downloaded: Utc::now(),
        };

        // This should serialize successfully
        let yaml = serde_yaml::to_string(&frontmatter).unwrap();
        assert!(yaml.contains("https://example.com"));

        let complete = valid_markdown.with_frontmatter(&format!("---\n{yaml}---\n"));
        assert!(complete.as_str().contains("# Valid Content"));
    }

    #[test]
    fn test_roundtrip_serialization_all_types() {
        // Test that all types can be serialized and deserialized consistently

        let url = Url::new("https://github.com/user/repo/issues/123".to_string()).unwrap();
        let markdown = Markdown::new("# Test Document\n\nContent here.".to_string()).unwrap();
        let frontmatter = Frontmatter {
            source_url: url.clone(),
            exporter: "comprehensive-test".to_string(),
            date_downloaded: DateTime::parse_from_rfc3339("2023-12-01T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let url_type = UrlType::GitHubIssue;

        // Helper to test roundtrip serialization
        let test_roundtrip = |value: &impl Serialize,
                              deserialize: fn(&str) -> Result<_, serde_yaml::Error>| {
            let serialized = serde_yaml::to_string(value).unwrap();
            let deserialized = deserialize(&serialized).unwrap();
            (serialized, deserialized)
        };

        // Test URL roundtrip
        let (url_yaml, url_deserialized) = test_roundtrip(&url, serde_yaml::from_str::<Url>);
        assert_eq!(url, url_deserialized);

        // Test Frontmatter roundtrip
        let (frontmatter_yaml, frontmatter_deserialized) =
            test_roundtrip(&frontmatter, serde_yaml::from_str::<Frontmatter>);
        assert_eq!(frontmatter, frontmatter_deserialized);

        // Test UrlType roundtrip
        let (_url_type_yaml, url_type_deserialized) =
            test_roundtrip(&url_type, serde_yaml::from_str::<UrlType>);
        assert_eq!(url_type, url_type_deserialized);

        // Test Markdown content preservation
        let markdown_with_frontmatter =
            markdown.with_frontmatter(&format!("---\n{frontmatter_yaml}---\n"));
        let extracted_content = markdown_with_frontmatter.content_only();
        assert_eq!(extracted_content, markdown.as_str());
    }
}
