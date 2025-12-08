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

mod helpers {
    use super::*;

    pub fn create_test_error_context() -> ErrorContext {
        ErrorContext::new("https://test.com", "test operation", "TestConverter")
    }

    pub fn assert_parse_error(result: Result<Markdown, MarkdownError>, expected_message: &str) {
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => assert_eq!(message, expected_message),
            err => panic!("Expected ParseError, got: {err:?}"),
        }
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
        assert_eq!(format!("{markdown}"), content);
    }

    #[test]
    fn test_markdown_creation_from_string() {
        let content = "# Test Content";
        let markdown = Markdown::from(content.to_string());
        assert_eq!(markdown.as_str(), content);
    }

    #[test]
    fn test_markdown_new_empty_content_fails() {
        let result = Markdown::new("".to_string());
        helpers::assert_parse_error(
            result,
            "Markdown content cannot be empty or whitespace-only",
        );
    }

    #[test]
    fn test_markdown_new_whitespace_only_fails() {
        let result = Markdown::new("   \n\t  \r\n  ".to_string());
        helpers::assert_parse_error(
            result,
            "Markdown content cannot be empty or whitespace-only",
        );
    }

    #[test]
    fn test_markdown_validation() {
        let valid_markdown = Markdown::from("# Valid Content".to_string());
        assert!(valid_markdown.validate().is_ok());

        let empty_markdown = Markdown::from("".to_string());
        assert!(empty_markdown.validate().is_err());

        let whitespace_markdown = Markdown::from("   \n  ".to_string());
        assert!(whitespace_markdown.validate().is_err());
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
    fn test_url_creation_invalid_protocol() {
        let invalid_urls = [
            "ftp://example.com",
            "mailto:test@example.com",
            "ws://example.com",
        ];

        for invalid_url in invalid_urls {
            let result = Url::new(invalid_url.to_string());
            assert!(result.is_err(), "Should reject URL: {invalid_url}");
            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.url, invalid_url);
                }
                _ => panic!("Expected ValidationError with InvalidUrl kind for: {invalid_url}"),
            }
        }
    }

    #[test]
    fn test_url_creation_incomplete() {
        let incomplete_urls = ["http://", "https://", "example.com", "www.example.com", ""];

        for incomplete_url in incomplete_urls {
            let result = Url::new(incomplete_url.to_string());
            assert!(result.is_err(), "Should reject URL: {incomplete_url}");
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

        // Test YAML serialization
        let yaml = serde_yaml::to_string(&url).unwrap();
        let deserialized: Url = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(url, deserialized);

        // Test JSON serialization
        let json = serde_json::to_string(&url).unwrap();
        let deserialized: Url = serde_json::from_str(&json).unwrap();
        assert_eq!(url, deserialized);
    }
}

/// Tests for the UrlType enumeration
mod url_type_tests {
    use super::*;

    #[test]
    fn test_url_type_display() {
        assert_eq!(format!("{}", UrlType::Html), "HTML");
        assert_eq!(format!("{}", UrlType::GoogleDocs), "Google Docs");
        assert_eq!(format!("{}", UrlType::GitHubIssue), "GitHub Issue");
    }

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
        let url_types = [UrlType::Html, UrlType::GoogleDocs, UrlType::GitHubIssue];

        for url_type in url_types {
            // Test YAML serialization
            let yaml = serde_yaml::to_string(&url_type).unwrap();
            let deserialized: UrlType = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(url_type, deserialized);

            // Test JSON serialization
            let json = serde_json::to_string(&url_type).unwrap();
            let deserialized: UrlType = serde_json::from_str(&json).unwrap();
            assert_eq!(url_type, deserialized);
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
        assert!((0..5).contains(&diff));
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

        // Test YAML serialization
        let yaml = serde_yaml::to_string(&context).unwrap();
        let deserialized: ErrorContext = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(context.url, deserialized.url);
        assert_eq!(context.operation, deserialized.operation);
        assert_eq!(context.converter_type, deserialized.converter_type);
        assert_eq!(context.additional_info, deserialized.additional_info);
        assert_eq!(context.timestamp, deserialized.timestamp);
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
        let context = helpers::create_test_error_context();

        // Test retryable network errors
        let timeout_error = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::Timeout,
            context: context.clone(),
        };
        assert!(timeout_error.is_retryable());
        assert!(timeout_error.is_recoverable());

        let connection_failed = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::ConnectionFailed,
            context: context.clone(),
        };
        assert!(connection_failed.is_retryable());

        let rate_limited = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::RateLimited,
            context: context.clone(),
        };
        assert!(rate_limited.is_retryable());

        // Test non-retryable network errors
        let dns_error = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::DnsResolution,
            context: context.clone(),
        };
        assert!(!dns_error.is_retryable());

        // Test server error logic
        let server_error_500 = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::ServerError(500),
            context: context.clone(),
        };
        assert!(server_error_500.is_retryable());

        let client_error_404 = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::ServerError(404),
            context,
        };
        assert!(!client_error_404.is_retryable());
    }

    #[test]
    fn test_auth_error_handling() {
        let context = helpers::create_test_error_context();

        let expired_token = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::TokenExpired,
            context: context.clone(),
        };
        assert!(expired_token.is_retryable());
        assert!(expired_token.is_recoverable());

        let missing_token = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::MissingToken,
            context: context.clone(),
        };
        assert!(!missing_token.is_retryable());
        assert!(missing_token.is_recoverable());

        let invalid_token = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::InvalidToken,
            context: context.clone(),
        };
        assert!(!invalid_token.is_retryable());

        let permission_denied = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::PermissionDenied,
            context,
        };
        assert!(!permission_denied.is_retryable());
    }

    #[test]
    fn test_content_error_recovery() {
        let context = helpers::create_test_error_context();

        let unsupported_format = MarkdownError::ContentError {
            kind: ContentErrorKind::UnsupportedFormat,
            context: context.clone(),
        };
        assert!(unsupported_format.is_recoverable());
        assert!(!unsupported_format.is_retryable());

        let empty_content = MarkdownError::ContentError {
            kind: ContentErrorKind::EmptyContent,
            context: context.clone(),
        };
        assert!(!empty_content.is_recoverable());

        let parsing_failed = MarkdownError::ContentError {
            kind: ContentErrorKind::ParsingFailed,
            context,
        };
        assert!(!parsing_failed.is_recoverable());
    }

    #[test]
    fn test_converter_error_recovery() {
        let context = helpers::create_test_error_context();

        let external_tool_failed = MarkdownError::ConverterError {
            kind: ConverterErrorKind::ExternalToolFailed,
            context: context.clone(),
        };
        assert!(external_tool_failed.is_recoverable());
        assert!(!external_tool_failed.is_retryable());

        let processing_error = MarkdownError::ConverterError {
            kind: ConverterErrorKind::ProcessingError,
            context: context.clone(),
        };
        assert!(processing_error.is_recoverable());

        let unsupported_operation = MarkdownError::ConverterError {
            kind: ConverterErrorKind::UnsupportedOperation,
            context,
        };
        assert!(unsupported_operation.is_recoverable());
    }

    #[test]
    fn test_error_suggestions_comprehensive() {
        let context = helpers::create_test_error_context();

        // Test validation error suggestions
        let validation_error = MarkdownError::ValidationError {
            kind: ValidationErrorKind::InvalidUrl,
            context: context.clone(),
        };
        let suggestions = validation_error.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("http")));

        // Test network error suggestions
        let network_error = MarkdownError::EnhancedNetworkError {
            kind: NetworkErrorKind::Timeout,
            context: context.clone(),
        };
        let suggestions = network_error.suggestions();
        assert!(suggestions
            .iter()
            .any(|s| s.contains("internet connection")));

        // Test auth error suggestions
        let auth_error = MarkdownError::AuthenticationError {
            kind: AuthErrorKind::MissingToken,
            context,
        };
        let suggestions = auth_error.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("authentication")));
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

        assert!(error.context().is_none());
        assert!(!error.is_retryable());
        assert!(!error.is_recoverable());

        let suggestions = error.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("content format")));
    }

    #[test]
    fn test_legacy_network_error() {
        let error = MarkdownError::NetworkError {
            message: "Connection timeout occurred".to_string(),
        };

        assert!(error.context().is_none());
        assert!(error.is_retryable()); // Should detect "timeout" in message
        assert!(error.is_recoverable());

        let suggestions = error.suggestions();
        assert!(suggestions
            .iter()
            .any(|s| s.contains("internet connection")));
    }

    #[test]
    fn test_legacy_invalid_url_error() {
        let error = MarkdownError::InvalidUrl {
            url: "not-a-url".to_string(),
        };

        assert!(error.context().is_none());
        assert!(!error.is_retryable());
        assert!(!error.is_recoverable());

        let suggestions = error.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("http")));
    }

    #[test]
    fn test_legacy_auth_error() {
        let error = MarkdownError::AuthError {
            message: "Invalid authentication token".to_string(),
        };

        assert!(error.context().is_none());
        assert!(!error.is_retryable());
        assert!(error.is_recoverable());

        let suggestions = error.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("authentication")));
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
    fn test_frontmatter_yaml_serialization() {
        let frontmatter = Frontmatter {
            source_url: Url::new("https://example.com".to_string()).unwrap(),
            exporter: "markdowndown".to_string(),
            date_downloaded: DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };

        let yaml = serde_yaml::to_string(&frontmatter).unwrap();
        let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(frontmatter, deserialized);
    }

    #[test]
    fn test_frontmatter_json_serialization() {
        let frontmatter = Frontmatter {
            source_url: Url::new("https://docs.google.com/document/d/123".to_string()).unwrap(),
            exporter: "test-exporter".to_string(),
            date_downloaded: Utc::now(),
        };

        let json = serde_json::to_string(&frontmatter).unwrap();
        let deserialized: Frontmatter = serde_json::from_str(&json).unwrap();
        assert_eq!(frontmatter, deserialized);
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
            // Well-formed URLs should always be accepted
            let result = Url::new(url_str.clone());
            if url_str.len() > 8 { // Minimum for "https://" + at least one char
                assert!(result.is_ok(), "Should accept well-formed URL: {url_str}");
            }
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

        // Test URL roundtrip
        let url_yaml = serde_yaml::to_string(&url).unwrap();
        let url_deserialized: Url = serde_yaml::from_str(&url_yaml).unwrap();
        assert_eq!(url, url_deserialized);

        // Test Frontmatter roundtrip
        let frontmatter_yaml = serde_yaml::to_string(&frontmatter).unwrap();
        let frontmatter_deserialized: Frontmatter =
            serde_yaml::from_str(&frontmatter_yaml).unwrap();
        assert_eq!(frontmatter, frontmatter_deserialized);

        // Test UrlType roundtrip
        let url_type = UrlType::GitHubIssue;
        let url_type_yaml = serde_yaml::to_string(&url_type).unwrap();
        let url_type_deserialized: UrlType = serde_yaml::from_str(&url_type_yaml).unwrap();
        assert_eq!(url_type, url_type_deserialized);

        // Test Markdown content preservation
        let markdown_with_frontmatter =
            markdown.with_frontmatter(&format!("---\n{frontmatter_yaml}---\n"));
        let extracted_content = markdown_with_frontmatter.content_only();
        assert_eq!(extracted_content, markdown.as_str());
    }
}
