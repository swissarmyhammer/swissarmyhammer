//! Property-based tests using proptest for robustness validation.
//!
//! This module uses property-based testing to verify that the library
//! behaves correctly with a wide range of generated inputs, helping
//! discover edge cases and ensure robustness.
//!
//! Note: Some tests are temporarily disabled due to API changes.

#![cfg(test)]

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{Converter, HtmlConverter, HtmlConverterConfig};
use markdowndown::detection::UrlDetector;
use markdowndown::types::{ErrorContext, Markdown, MarkdownError, Url, UrlType};
use markdowndown::{detect_url_type, MarkdownDown};
use proptest::prelude::*;
use std::time::Duration;

/// Custom strategy for generating valid HTTP/HTTPS URLs
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

/// Strategy for generating potentially invalid URLs to test error handling
fn arbitrary_url_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9:/.?#&=-]{1,100}").unwrap()
}

/// Strategy for generating markdown content with various structures
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

            // Ensure content is never empty by adding default content if needed
            if content.trim().is_empty() {
                content = "Default content".to_string();
            }

            content
        })
}

/// Property tests for URL validation
mod url_validation_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_valid_urls_create_successfully(url in valid_http_url_strategy()) {
            let result = Url::new(url.clone());
            prop_assert!(result.is_ok(), "Failed to create URL from: {}", url);

            let url_obj = result.unwrap();
            prop_assert_eq!(url_obj.as_str(), &url);
        }

        #[test]
        fn test_url_as_str_roundtrip(url in valid_http_url_strategy()) {
            if let Ok(url_obj) = Url::new(url.clone()) {
                let as_str = url_obj.as_str();
                prop_assert_eq!(as_str, &url);

                // Should be able to create another URL from as_str
                let url_obj2 = Url::new(as_str.to_string());
                prop_assert!(url_obj2.is_ok());
                let url_obj2_unwrapped = url_obj2.unwrap();
                prop_assert_eq!(url_obj2_unwrapped.as_str(), as_str);
            }
        }

        #[test]
        fn test_url_clone_equality(url in valid_http_url_strategy()) {
            if let Ok(url_obj) = Url::new(url) {
                let cloned = url_obj.clone();
                prop_assert_eq!(url_obj.as_str(), cloned.as_str());
                prop_assert_eq!(url_obj, cloned);
            }
        }

        #[test]
        fn test_arbitrary_urls_handled_gracefully(url in arbitrary_url_strategy()) {
            let result = Url::new(url);
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
    }
}

/// Property tests for markdown content validation
mod markdown_validation_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_markdown_content_preservation(content in markdown_content_strategy()) {
            let result = Markdown::new(content.clone());
            prop_assert!(result.is_ok(), "Failed to create Markdown from content");

            let markdown = result.unwrap();
            let retrieved_content = markdown.as_str();
            prop_assert_eq!(retrieved_content, &content);
        }

        #[test]
        fn test_markdown_content_only_no_frontmatter(content in markdown_content_strategy()) {
            if let Ok(markdown) = Markdown::new(content.clone()) {
                let content_only = markdown.content_only();
                prop_assert_eq!(content_only, content);

                // With no frontmatter, frontmatter() should return None
                prop_assert!(markdown.frontmatter().is_none());
            }
        }

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
    }

    proptest! {
        #[test]
        fn test_markdown_clone_equality(content in markdown_content_strategy()) {
            if let Ok(markdown) = Markdown::new(content) {
                let cloned = markdown.clone();
                prop_assert_eq!(markdown.as_str(), cloned.as_str());
                prop_assert_eq!(markdown, cloned);
            }
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
            let detector = UrlDetector::new();

            // Detection should be deterministic
            let result1 = detector.detect_type(&url);
            let result2 = detector.detect_type(&url);

            prop_assert_eq!(result1.is_ok(), result2.is_ok());
            if result1.is_ok() && result2.is_ok() {
                prop_assert_eq!(result1.unwrap(), result2.unwrap());
            }
        }

        #[test]
        fn test_url_normalization_idempotent(url in valid_http_url_strategy()) {
            let detector = UrlDetector::new();

            if let Ok(normalized1) = detector.normalize_url(&url) {
                // Normalizing already normalized URL should be idempotent
                if let Ok(normalized2) = detector.normalize_url(&normalized1) {
                    prop_assert_eq!(normalized1, normalized2);
                }
            }
        }

        #[test]
        fn test_google_docs_urls_detected(
            doc_id in prop::string::string_regex("[a-zA-Z0-9_-]{10,50}").unwrap()
        ) {
            let url = format!("https://docs.google.com/document/d/{doc_id}/edit");

            match detect_url_type(&url) {
                Ok(url_type) => {
                    prop_assert_eq!(url_type, UrlType::GoogleDocs);
                }
                Err(_) => {
                    // Some doc IDs might be invalid, which is acceptable
                }
            }
        }

        #[test]
        fn test_github_issue_urls_detected(
            owner in prop::string::string_regex("[a-zA-Z0-9_-]{1,20}").unwrap(),
            repo in prop::string::string_regex("[a-zA-Z0-9_-]{1,30}").unwrap(),
            issue_num in 1u32..100000u32
        ) {
            let url = format!("https://github.com/{owner}/{repo}/issues/{issue_num}");

            match detect_url_type(&url) {
                Ok(url_type) => {
                    prop_assert_eq!(url_type, UrlType::GitHubIssue);
                }
                Err(_) => {
                    // Some URLs might be invalid, which is acceptable
                }
            }
        }


        #[test]
        fn test_html_urls_as_fallback(url in valid_http_url_strategy()) {
            // URLs that don't match specific patterns should be detected as HTML
            if !url.contains("docs.google.com") &&
               !url.contains("github.com") &&
               !url.contains("sharepoint.com") {
                match detect_url_type(&url) {
                    Ok(url_type) => {
                        prop_assert_eq!(url_type, UrlType::Html);
                    }
                    Err(_) => {
                        // Some URLs might be invalid, which is acceptable
                    }
                }
            }
        }
    }
}

/// Property tests for configuration handling
mod configuration_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_config_timeout_values(timeout_secs in 1u64..3600u64) {
            let config = Config::builder()
                .timeout_seconds(timeout_secs)
                .build();

            prop_assert_eq!(config.http.timeout, Duration::from_secs(timeout_secs));
        }

        #[test]
        fn test_config_retry_values(max_retries in 0u32..20u32) {
            let config = Config::builder()
                .max_retries(max_retries)
                .build();

            prop_assert_eq!(config.http.max_retries, max_retries);
        }

        #[test]
        fn test_config_user_agent_preservation(
            user_agent in prop::string::string_regex("[a-zA-Z0-9_/. -]{1,100}").unwrap()
        ) {
            let config = Config::builder()
                .user_agent(&user_agent)
                .build();

            prop_assert_eq!(config.http.user_agent, user_agent);
        }

        #[test]
        fn test_config_token_preservation(
            github_token in prop::option::of(prop::string::string_regex("[a-zA-Z0-9_]{10,50}").unwrap()),
            google_api_key in prop::option::of(prop::string::string_regex("[a-zA-Z0-9_]{10,50}").unwrap())
        ) {
            let mut builder = Config::builder();

            if let Some(ref token) = github_token {
                builder = builder.github_token(token);
            }
            if let Some(ref key) = google_api_key {
                builder = builder.google_api_key(key);
            }

            let config = builder.build();

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
                0..10
            )
        ) {
            let mut builder = Config::builder();

            for (key, value) in &fields {
                builder = builder.custom_frontmatter_field(key, value);
            }

            let config = builder.build();

            prop_assert_eq!(config.output.custom_frontmatter_fields.len(), fields.len());
            for (i, (key, value)) in fields.iter().enumerate() {
                prop_assert_eq!(&config.output.custom_frontmatter_fields[i].0, key);
                prop_assert_eq!(&config.output.custom_frontmatter_fields[i].1, value);
            }
        }

        #[test]
        fn test_config_builder_chaining(
            timeout in 1u64..100u64,
            retries in 0u32..10u32,
            include_frontmatter in any::<bool>(),
            normalize_whitespace in any::<bool>()
        ) {
            let config = Config::builder()
                .timeout_seconds(timeout)
                .max_retries(retries)
                .include_frontmatter(include_frontmatter)
                .normalize_whitespace(normalize_whitespace)
                .build();

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

            let display_string = format!("{error}");
            prop_assert!(display_string.contains(&message));

            let debug_string = format!("{error:?}");
            prop_assert!(debug_string.contains(&message));
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
            network_status in 400u16..600u16
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
                500..=503 | 429 => prop_assert!(is_recoverable),
                400..=499 => prop_assert!(!is_recoverable),
                _ => {
                    // Other codes may or may not be recoverable based on implementation
                }
            }
        }
    }
}

/// Property tests for HTML converter configuration
mod html_converter_properties {
    use super::*;

    proptest! {
        #[test]
        fn test_html_converter_config_validation(
            max_line_width in 20usize..500usize,
            max_blank_lines in 0usize..20usize,
            remove_scripts in any::<bool>(),
            remove_navigation in any::<bool>(),
            remove_sidebars in any::<bool>(),
            remove_ads in any::<bool>()
        ) {
            let config = HtmlConverterConfig {
                max_line_width,
                remove_scripts_styles: remove_scripts,
                remove_navigation,
                remove_sidebars,
                remove_ads,
                max_blank_lines,
            };

            // Configuration should be stored correctly
            prop_assert_eq!(config.max_line_width, max_line_width);
            prop_assert_eq!(config.remove_scripts_styles, remove_scripts);
            prop_assert_eq!(config.remove_navigation, remove_navigation);
            prop_assert_eq!(config.remove_sidebars, remove_sidebars);
            prop_assert_eq!(config.remove_ads, remove_ads);
            prop_assert_eq!(config.max_blank_lines, max_blank_lines);

            // Should be able to create converter with this config
            let client = HttpClient::new();
            let output_config = markdowndown::config::OutputConfig::default();
            let converter = HtmlConverter::with_config(client, config.clone(), output_config);
            prop_assert_eq!(converter.name(), "HTML");
        }

        #[test]
        fn test_html_converter_config_clone(
            max_line_width in 50usize..200usize,
            max_blank_lines in 1usize..10usize
        ) {
            let original_config = HtmlConverterConfig {
                max_line_width,
                remove_scripts_styles: true,
                remove_navigation: false,
                remove_sidebars: true,
                remove_ads: false,
                max_blank_lines,
            };

            let cloned_config = original_config.clone();

            prop_assert_eq!(original_config.max_line_width, cloned_config.max_line_width);
            prop_assert_eq!(original_config.remove_scripts_styles, cloned_config.remove_scripts_styles);
            prop_assert_eq!(original_config.remove_navigation, cloned_config.remove_navigation);
            prop_assert_eq!(original_config.remove_sidebars, cloned_config.remove_sidebars);
            prop_assert_eq!(original_config.remove_ads, cloned_config.remove_ads);
            prop_assert_eq!(original_config.max_blank_lines, cloned_config.max_blank_lines);
        }
    }
}

/// Property tests for MarkdownDown main API
mod markdowndown_api_properties {
    use super::*;

    #[test]
    fn test_markdowndown_supported_types_consistency() {
        let md1 = MarkdownDown::new();
        let md2 = MarkdownDown::default();
        let md3 = MarkdownDown::with_config(Config::default());

        let types1 = md1.supported_types();
        let types2 = md2.supported_types();
        let types3 = md3.supported_types();

        // All instances should report the same supported types (order may vary)
        use std::collections::HashSet;
        let set1: HashSet<_> = types1.iter().collect();
        let set2: HashSet<_> = types2.iter().collect();
        let set3: HashSet<_> = types3.iter().collect();

        assert_eq!(set1, set2);
        assert_eq!(set2, set3);

        // Should include the core types
        assert!(types1.contains(&UrlType::Html));
        assert!(types1.contains(&UrlType::GoogleDocs));
        assert!(types1.contains(&UrlType::GitHubIssue));
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
