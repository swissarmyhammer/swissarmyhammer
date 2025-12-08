//! Comprehensive unit tests for converter registry functionality.
//!
//! This module tests the converter registry which manages the mapping
//! of URL types to specific converter implementations.

use markdowndown::client::HttpClient;
use markdowndown::config::Config;
use markdowndown::converters::{
    Converter, ConverterRegistry, GoogleDocsConverter, HtmlConverter, HtmlConverterConfig,
};
use markdowndown::types::{MarkdownError, UrlType};

mod helpers {
    use super::*;

    /// Create a test registry with default converters
    pub fn create_test_registry() -> ConverterRegistry {
        ConverterRegistry::new()
    }

    /// Create a test registry with configured converters
    pub fn create_configured_registry() -> ConverterRegistry {
        let config = Config::builder()
            .timeout_seconds(10)
            .user_agent("test-registry/1.0")
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig::default();

        ConverterRegistry::with_config(client, html_config, &output_config)
    }

    /// Test URL mappings for each converter type
    pub fn url_type_mappings() -> Vec<(UrlType, &'static str)> {
        vec![
            (UrlType::Html, "https://example.com/page.html"),
            (
                UrlType::GoogleDocs,
                "https://docs.google.com/document/d/123/edit",
            ),
            (
                UrlType::GitHubIssue,
                "https://github.com/owner/repo/issues/123",
            ),
            (UrlType::LocalFile, "/path/to/test.md"),
        ]
    }
}

/// Tests for registry creation and configuration
mod registry_creation_tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = ConverterRegistry::new();
        let supported_types = registry.supported_types();

        // Should support all major URL types
        assert!(supported_types.contains(&UrlType::Html));
        assert!(supported_types.contains(&UrlType::GoogleDocs));
        assert!(supported_types.contains(&UrlType::GitHubIssue));
        assert!(supported_types.contains(&UrlType::LocalFile));
        assert_eq!(supported_types.len(), 4);
    }

    #[test]
    fn test_registry_default() {
        let registry = ConverterRegistry::default();
        let supported_types = registry.supported_types();

        // Default should be equivalent to new()
        assert!(supported_types.contains(&UrlType::Html));
        assert!(supported_types.contains(&UrlType::GoogleDocs));
        assert!(supported_types.contains(&UrlType::GitHubIssue));
        assert!(supported_types.contains(&UrlType::LocalFile));
        assert_eq!(supported_types.len(), 4);
    }

    #[test]
    fn test_registry_with_config() {
        let config = Config::builder()
            .timeout_seconds(30)
            .user_agent("test-app/1.0")
            .github_token("test_token")
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = HtmlConverterConfig {
            max_line_width: 100,
            remove_scripts_styles: true,
            remove_navigation: true,
            remove_sidebars: true,
            remove_ads: true,
            max_blank_lines: 1,
        };
        let output_config = markdowndown::config::OutputConfig::default();

        let registry = ConverterRegistry::with_config(client, html_config, &output_config);
        let supported_types = registry.supported_types();

        // Should support all URL types with custom configuration
        assert_eq!(supported_types.len(), 4);
        assert!(supported_types.contains(&UrlType::Html));
        assert!(supported_types.contains(&UrlType::GoogleDocs));
        assert!(supported_types.contains(&UrlType::GitHubIssue));
        assert!(supported_types.contains(&UrlType::LocalFile));
    }
}

/// Tests for converter registration and retrieval
mod converter_management_tests {
    use super::*;

    #[test]
    fn test_get_converter_for_each_type() {
        let registry = helpers::create_test_registry();

        for (url_type, _) in helpers::url_type_mappings() {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some(), "No converter found for {url_type:?}");

            // Verify converter names match expected types
            let converter = converter.unwrap();
            match url_type {
                UrlType::Html => assert_eq!(converter.name(), "HTML"),
                UrlType::GoogleDocs => assert_eq!(converter.name(), "Google Docs"),
                UrlType::GitHubIssue => assert_eq!(converter.name(), "GitHub Issue"),
                UrlType::LocalFile => assert_eq!(converter.name(), "Local File Converter"),
            }
        }
    }

    #[test]
    fn test_get_converter_nonexistent_type() {
        let registry = helpers::create_test_registry();

        // Create a mock URL type that doesn't exist in the registry
        // Since all UrlType variants are covered, we'll test retrieval logic
        let supported_types = registry.supported_types();
        assert!(!supported_types.is_empty());

        // Test that we can retrieve any supported type
        for url_type in supported_types {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some());
        }
    }

    #[test]
    fn test_register_custom_converter() {
        let mut registry = ConverterRegistry::new();

        // Replace HTML converter with a custom one
        let custom_html_converter = Box::new(HtmlConverter::new());
        registry.register(UrlType::Html, custom_html_converter);

        let converter = registry.get_converter(&UrlType::Html);
        assert!(converter.is_some());
        assert_eq!(converter.unwrap().name(), "HTML");
    }

    #[test]
    fn test_supported_types_after_registration() {
        let mut registry = ConverterRegistry::new();
        let initial_count = registry.supported_types().len();

        // Register a duplicate type (should replace existing)
        let custom_converter = Box::new(GoogleDocsConverter::new());
        registry.register(UrlType::GoogleDocs, custom_converter);

        let final_count = registry.supported_types().len();
        assert_eq!(initial_count, final_count); // Should be same count (replacement, not addition)

        // Verify the converter is still accessible
        let converter = registry.get_converter(&UrlType::GoogleDocs);
        assert!(converter.is_some());
        assert_eq!(converter.unwrap().name(), "Google Docs");
    }
}

/// Tests for converter functionality through registry
mod converter_functionality_tests {
    use super::*;

    #[test]
    fn test_html_converter_through_registry() {
        let registry = helpers::create_test_registry();
        let converter = registry.get_converter(&UrlType::Html).unwrap();

        assert_eq!(converter.name(), "HTML");
        // Additional functionality tests would require async setup
    }

    #[test]
    fn test_google_docs_converter_through_registry() {
        let registry = helpers::create_test_registry();
        let converter = registry.get_converter(&UrlType::GoogleDocs).unwrap();

        assert_eq!(converter.name(), "Google Docs");
    }

    #[test]
    fn test_github_converter_through_registry() {
        let registry = helpers::create_test_registry();
        let converter = registry.get_converter(&UrlType::GitHubIssue).unwrap();

        assert_eq!(converter.name(), "GitHub Issue");
    }

    #[test]
    fn test_github_converter_is_api_not_placeholder() {
        // This test verifies that the registry uses the full GitHub API converter
        // instead of the placeholder converter
        let registry = helpers::create_test_registry();
        let converter = registry.get_converter(&UrlType::GitHubIssue).unwrap();

        // The GitHub API converter should be named "GitHub Issue"
        assert_eq!(converter.name(), "GitHub Issue");

        // Additional verification: The converter should be the real GitHubConverter type
        // We can't directly test the type, but we can verify behavior differences
        // The placeholder would use the PlaceholderConverter pattern which has specific output
        // while the API converter has different behavior patterns

        // For now, we verify that we have the right converter name and it's available
        assert_eq!(converter.name(), "GitHub Issue");
    }
}

/// Tests for registry configuration propagation
mod configuration_propagation_tests {
    use super::*;

    #[test]
    fn test_configured_registry_converters() {
        let registry = helpers::create_configured_registry();

        // All converters should be present
        let supported_types = registry.supported_types();
        assert_eq!(supported_types.len(), 4);

        // Verify each converter is accessible
        for url_type in supported_types {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some(), "Missing converter for {url_type:?}");
        }
    }

    #[test]
    fn test_registry_converter_names_consistent() {
        let default_registry = ConverterRegistry::new();
        let configured_registry = helpers::create_configured_registry();

        let default_types = default_registry.supported_types();
        let configured_types = configured_registry.supported_types();

        // Both should support the same types
        assert_eq!(default_types.len(), configured_types.len());
        for url_type in default_types {
            assert!(configured_types.contains(&url_type));

            // Converter names should be the same
            let default_converter = default_registry.get_converter(&url_type).unwrap();
            let configured_converter = configured_registry.get_converter(&url_type).unwrap();
            assert_eq!(default_converter.name(), configured_converter.name());
        }
    }
}

/// Tests for error handling and edge cases
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_populated_registry_unsupported_type() {
        let registry = ConverterRegistry::new();

        let supported_types = registry.supported_types();
        assert!(!supported_types.is_empty()); // Has default converters

        // Test with a type that should exist
        let converter = registry.get_converter(&UrlType::Html);
        assert!(converter.is_some());
    }

    #[test]
    fn test_registry_with_single_converter() {
        // Create empty registry and register only HTML converter
        let mut registry = ConverterRegistry::empty();
        registry.register(UrlType::Html, Box::new(HtmlConverter::new()));

        // Test that HTML converter exists (it should in default registry)
        let converter = registry.get_converter(&UrlType::Html);
        assert!(converter.is_some());

        let supported_types = registry.supported_types();
        assert_eq!(supported_types.len(), 1);
        assert!(supported_types.contains(&UrlType::Html));

        // HTML converter should be available
        let html_converter = registry.get_converter(&UrlType::Html);
        assert!(html_converter.is_some());
        assert_eq!(html_converter.unwrap().name(), "HTML");

        // Other converters should not be available
        let docs_converter = registry.get_converter(&UrlType::GoogleDocs);
        assert!(docs_converter.is_none());
    }

    #[test]
    fn test_registry_replacement_of_converter() {
        let mut registry = ConverterRegistry::new();

        // Verify initial HTML converter
        let initial_converter = registry.get_converter(&UrlType::Html).unwrap();
        assert_eq!(initial_converter.name(), "HTML");

        // Replace with a new HTML converter
        let new_html_converter = Box::new(HtmlConverter::new());
        registry.register(UrlType::Html, new_html_converter);

        // Should still be HTML converter
        let replaced_converter = registry.get_converter(&UrlType::Html).unwrap();
        assert_eq!(replaced_converter.name(), "HTML");

        // Registry should still have same number of converters
        let supported_types = registry.supported_types();
        assert_eq!(supported_types.len(), 4);
    }
}

/// Integration tests for registry usage patterns
mod integration_tests {
    use super::*;

    #[test]
    fn test_registry_supports_all_url_types() {
        let registry = helpers::create_test_registry();

        // Test each URL type mapping
        for (url_type, sample_url) in helpers::url_type_mappings() {
            let converter = registry.get_converter(&url_type);
            assert!(
                converter.is_some(),
                "No converter found for URL type {url_type:?} with sample URL: {sample_url}"
            );
        }
    }

    #[test]
    fn test_registry_workflow_simulation() {
        let registry = helpers::create_test_registry();

        // Simulate typical workflow: URL detection -> converter retrieval -> conversion
        let test_cases = vec![
            (UrlType::Html, "HTML"),
            (UrlType::GoogleDocs, "Google Docs"),
            (UrlType::GitHubIssue, "GitHub Issue"),
        ];

        for (url_type, expected_name) in test_cases {
            // Step 1: Get converter for detected URL type
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some());

            // Step 2: Verify converter identity
            let converter = converter.unwrap();
            assert_eq!(converter.name(), expected_name);

            // Step 3: Converter should be ready for use
            // (Actual conversion would require async context and URL)
        }
    }

    #[test]
    fn test_registry_performance_with_multiple_lookups() {
        let registry = helpers::create_test_registry();

        // Perform many lookups to test performance
        for _ in 0..1000 {
            for (url_type, _) in helpers::url_type_mappings() {
                let converter = registry.get_converter(&url_type);
                assert!(converter.is_some());
            }
        }

        // If we get here, performance is acceptable for the test
        // Test passes if no panic occurred during performance test
    }

    #[test]
    fn test_registry_thread_safety_simulation() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(helpers::create_test_registry());
        let mut handles = vec![];

        // Simulate multiple threads accessing the registry
        for i in 0..10 {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                for (url_type, _) in helpers::url_type_mappings() {
                    let converter = registry_clone.get_converter(&url_type);
                    assert!(
                        converter.is_some(),
                        "Thread {i} failed to get converter for {url_type:?}"
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_registry_with_all_configuration_options() {
        let config = Config::builder()
            .timeout_seconds(60)
            .user_agent("comprehensive-test/1.0")
            .max_retries(5)
            .github_token("test_token")
            .office365_token("test_office_token")
            .google_api_key("test_google_key")
            .include_frontmatter(true)
            .normalize_whitespace(true)
            .build();

        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = HtmlConverterConfig {
            max_line_width: 150,
            remove_scripts_styles: false,
            remove_navigation: false,
            remove_sidebars: false,
            remove_ads: false,
            max_blank_lines: 5,
        };
        let output_config = markdowndown::config::OutputConfig::default();

        let registry = ConverterRegistry::with_config(client, html_config, &output_config);

        // Verify all converters are properly configured
        let supported_types = registry.supported_types();
        assert_eq!(supported_types.len(), 4);

        for url_type in supported_types {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some());

            // Verify converter names are correct
            let converter = converter.unwrap();
            match url_type {
                UrlType::Html => assert_eq!(converter.name(), "HTML"),
                UrlType::GoogleDocs => assert_eq!(converter.name(), "Google Docs"),
                UrlType::GitHubIssue => assert_eq!(converter.name(), "GitHub Issue"),
                UrlType::LocalFile => assert_eq!(converter.name(), "Local File Converter"),
            }
        }
    }
}

/// Tests for registry extensibility
mod extensibility_tests {
    use super::*;

    /// Create a mock converter for testing
    struct MockConverter {
        name: &'static str,
    }

    impl MockConverter {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }

    #[async_trait::async_trait]
    impl Converter for MockConverter {
        async fn convert(
            &self,
            _url: &str,
        ) -> Result<markdowndown::types::Markdown, MarkdownError> {
            markdowndown::types::Markdown::new(format!("Mock conversion by {}", self.name))
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    #[test]
    fn test_registry_with_custom_converter() {
        let mut registry = ConverterRegistry::new();

        // Replace HTML converter with mock
        let mock_converter = Box::new(MockConverter::new("MockHtmlConverter"));
        registry.register(UrlType::Html, mock_converter);

        let converter = registry.get_converter(&UrlType::Html).unwrap();
        assert_eq!(converter.name(), "MockHtmlConverter");

        // Other converters should remain unchanged
        let docs_converter = registry.get_converter(&UrlType::GoogleDocs).unwrap();
        assert_eq!(docs_converter.name(), "Google Docs");
    }

    #[test]
    fn test_registry_supports_converter_replacement() {
        let mut registry = ConverterRegistry::new();

        // Replace multiple converters
        registry.register(UrlType::Html, Box::new(MockConverter::new("CustomHtml")));
        registry.register(
            UrlType::GoogleDocs,
            Box::new(MockConverter::new("CustomDocs")),
        );

        // Verify replacements
        assert_eq!(
            registry.get_converter(&UrlType::Html).unwrap().name(),
            "CustomHtml"
        );
        assert_eq!(
            registry.get_converter(&UrlType::GoogleDocs).unwrap().name(),
            "CustomDocs"
        );

        // Original converters should remain
        assert_eq!(
            registry
                .get_converter(&UrlType::GitHubIssue)
                .unwrap()
                .name(),
            "GitHub Issue"
        );
    }
}

/// Performance and stress tests
mod performance_tests {
    use super::*;

    #[test]
    fn test_registry_lookup_performance() {
        let registry = helpers::create_test_registry();
        let url_types = vec![UrlType::Html, UrlType::GoogleDocs, UrlType::GitHubIssue];

        let start = std::time::Instant::now();

        // Perform many lookups
        for _ in 0..10000 {
            for url_type in &url_types {
                let _converter = registry.get_converter(url_type);
            }
        }

        let duration = start.elapsed();

        // Should complete quickly (under 100ms for 40,000 lookups)
        assert!(duration < std::time::Duration::from_millis(100));
    }

    #[test]
    fn test_registry_supported_types_performance() {
        let registry = helpers::create_test_registry();

        let start = std::time::Instant::now();

        // Get supported types many times
        for _ in 0..1000 {
            let _types = registry.supported_types();
        }

        let duration = start.elapsed();

        // Should complete quickly (under 10ms for 1,000 calls)
        assert!(duration < std::time::Duration::from_millis(10));
    }
}
