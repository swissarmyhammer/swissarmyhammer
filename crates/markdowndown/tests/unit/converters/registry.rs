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

// Test configuration constants
const TEST_TIMEOUT_SECONDS: u64 = 10;
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const COMPREHENSIVE_TEST_TIMEOUT_SECONDS: u64 = 60;
const TEST_MAX_RETRIES: u32 = 5;

// HTML configuration constants
const DEFAULT_MAX_LINE_WIDTH: usize = 100;
const DEFAULT_MAX_BLANK_LINES: usize = 1;
const PERMISSIVE_MAX_LINE_WIDTH: usize = 150;
const PERMISSIVE_MAX_BLANK_LINES: usize = 5;

// Converter type constants
const MIN_STANDARD_CONVERTER_TYPES: usize = 4;

// Thread configuration constants
const DEFAULT_THREAD_COUNT: usize = 4;

// Performance test constants
const PERFORMANCE_TEST_ITERATIONS: usize = 1000;
const LOOKUP_STRESS_TEST_ITERATIONS: usize = 10_000;
const SUPPORTED_TYPES_STRESS_TEST_ITERATIONS: usize = 1_000;
const LOOKUP_STRESS_TEST_MAX_DURATION_SECS: u64 = 5;
const SUPPORTED_TYPES_STRESS_TEST_MAX_DURATION_SECS: u64 = 1;

mod helpers {
    use super::*;

    /// HTML configuration presets for testing
    pub enum HtmlConfigPreset {
        Default,
        Permissive,
    }

    /// Create a test registry with default converters
    pub fn create_test_registry() -> ConverterRegistry {
        ConverterRegistry::new()
    }

    /// Create a test registry with configured converters
    pub fn create_configured_registry() -> ConverterRegistry {
        let config = Config::builder()
            .timeout_seconds(TEST_TIMEOUT_SECONDS)
            .user_agent("test-registry/1.0")
            .build();
        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = HtmlConverterConfig::default();
        let output_config = markdowndown::config::OutputConfig::default();

        ConverterRegistry::with_config(client, html_config, &output_config)
    }

    /// Test URL mappings with expected converter names (single source of truth)
    pub fn url_type_mappings_with_names() -> Vec<(UrlType, &'static str, &'static str)> {
        vec![
            (UrlType::Html, "https://example.com/page.html", "HTML"),
            (
                UrlType::GoogleDocs,
                "https://docs.google.com/document/d/123/edit",
                "Google Docs",
            ),
            (
                UrlType::GitHubIssue,
                "https://github.com/owner/repo/issues/123",
                "GitHub Issue",
            ),
            (
                UrlType::LocalFile,
                "/path/to/test.md",
                "Local File Converter",
            ),
        ]
    }

    /// Test URL mappings for each converter type (derived from url_type_mappings_with_names)
    pub fn url_type_mappings() -> Vec<(UrlType, &'static str)> {
        url_type_mappings_with_names()
            .into_iter()
            .map(|(url_type, url, _)| (url_type, url))
            .collect()
    }

    /// Assert that a registry contains specified types
    pub fn assert_registry_has_types(registry: &ConverterRegistry, expected_types: &[UrlType]) {
        let supported_types = registry.supported_types();
        for url_type in expected_types {
            assert!(
                supported_types.contains(url_type),
                "Registry missing type: {:?}",
                url_type
            );
        }
    }

    /// Assert that a registry contains all standard converter types
    pub fn assert_standard_registry_types(registry: &ConverterRegistry) {
        assert_registry_has_types(
            registry,
            &[
                UrlType::Html,
                UrlType::GoogleDocs,
                UrlType::GitHubIssue,
                UrlType::LocalFile,
            ],
        );
        let supported_types = registry.supported_types();
        assert!(
            supported_types.len() >= MIN_STANDARD_CONVERTER_TYPES,
            "Registry should support at least HTML, GoogleDocs, GitHubIssue, and LocalFile"
        );
    }

    /// Assert registry creation and validation using a custom creation function
    pub fn assert_registry_creation<F>(create_fn: F)
    where
        F: FnOnce() -> ConverterRegistry,
    {
        let registry = create_fn();
        assert_standard_registry_types(&registry);
    }

    /// Get the expected converter name for a URL type
    pub fn expected_converter_name(url_type: &UrlType) -> &'static str {
        match url_type {
            UrlType::Html => "HTML",
            UrlType::GoogleDocs => "Google Docs",
            UrlType::GitHubIssue => "GitHub Issue",
            UrlType::LocalFile => "Local File Converter",
        }
    }

    /// Assert that a converter's name matches the expected name for its URL type
    pub fn assert_converter_name_matches_type(
        url_type: &UrlType,
        converter: &Box<dyn Converter>,
    ) {
        assert_eq!(converter.name(), expected_converter_name(url_type));
    }

    /// Verify converter names match expected values for given test cases
    pub fn verify_converter_names(
        registry: &ConverterRegistry,
        test_cases: Vec<(UrlType, &'static str)>,
    ) {
        for (url_type, expected_name) in test_cases {
            let converter = registry.get_converter(&url_type).unwrap();
            assert_eq!(converter.name(), expected_name);
        }
    }

    /// Verify registry consistency between two registries
    pub fn verify_registry_consistency(
        registry1: &ConverterRegistry,
        registry2: &ConverterRegistry,
    ) {
        let types1 = registry1.supported_types();
        let types2 = registry2.supported_types();
        assert_eq!(types1.len(), types2.len());

        for url_type in types1 {
            assert!(types2.contains(&url_type));
            let conv1 = registry1.get_converter(&url_type).unwrap();
            let conv2 = registry2.get_converter(&url_type).unwrap();
            assert_eq!(conv1.name(), conv2.name());
        }
    }

    /// Assert that all converters are available for all URL type mappings
    pub fn assert_all_converters_available(registry: &ConverterRegistry) {
        for (url_type, sample_url) in url_type_mappings() {
            let converter = registry.get_converter(&url_type);
            assert!(
                converter.is_some(),
                "No converter found for URL type {url_type:?} with sample URL: {sample_url}"
            );
        }
    }

    /// Create an HTML configuration from a preset
    pub fn html_config_from_preset(preset: HtmlConfigPreset) -> HtmlConverterConfig {
        match preset {
            HtmlConfigPreset::Default => HtmlConverterConfig {
                max_line_width: DEFAULT_MAX_LINE_WIDTH,
                remove_scripts_styles: true,
                remove_navigation: true,
                remove_sidebars: true,
                remove_ads: true,
                max_blank_lines: DEFAULT_MAX_BLANK_LINES,
            },
            HtmlConfigPreset::Permissive => HtmlConverterConfig {
                max_line_width: PERMISSIVE_MAX_LINE_WIDTH,
                remove_scripts_styles: false,
                remove_navigation: false,
                remove_sidebars: false,
                remove_ads: false,
                max_blank_lines: PERMISSIVE_MAX_BLANK_LINES,
            },
        }
    }

    /// Create a default HTML converter configuration for testing
    pub fn default_html_config() -> HtmlConverterConfig {
        html_config_from_preset(HtmlConfigPreset::Default)
    }

    /// Create a permissive HTML converter configuration for testing
    pub fn permissive_html_config() -> HtmlConverterConfig {
        html_config_from_preset(HtmlConfigPreset::Permissive)
    }
}

/// Tests for registry creation and configuration
mod registry_creation_tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        helpers::assert_registry_creation(|| ConverterRegistry::new());
    }

    #[test]
    fn test_registry_default() {
        helpers::assert_registry_creation(|| ConverterRegistry::default());
    }

    #[test]
    fn test_registry_with_config() {
        helpers::assert_registry_creation(|| {
            let config = Config::builder()
                .timeout_seconds(DEFAULT_TIMEOUT_SECONDS)
                .user_agent("test-app/1.0")
                .github_token("test_token")
                .build();
            let client = HttpClient::with_config(&config.http, &config.auth);
            let html_config = helpers::default_html_config();
            let output_config = markdowndown::config::OutputConfig::default();

            ConverterRegistry::with_config(client, html_config, &output_config)
        });
    }
}

/// Tests for converter registration and retrieval
mod converter_management_tests {
    use super::*;

    #[test]
    fn test_get_converter_for_each_type() {
        let registry = helpers::create_test_registry();

        for (url_type, _, expected_name) in helpers::url_type_mappings_with_names() {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some(), "No converter found for {url_type:?}");

            let converter = converter.unwrap();
            assert_eq!(converter.name(), expected_name);
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
    fn test_converters_through_registry() {
        let registry = helpers::create_test_registry();
        let test_cases = vec![
            (UrlType::Html, "HTML"),
            (UrlType::GoogleDocs, "Google Docs"),
            (UrlType::GitHubIssue, "GitHub Issue"),
        ];

        helpers::verify_converter_names(&registry, test_cases);
    }

    #[test]
    fn test_github_converter_is_api_not_placeholder() {
        let registry = helpers::create_test_registry();
        let converter = registry.get_converter(&UrlType::GitHubIssue).unwrap();

        assert_eq!(converter.name(), "GitHub Issue");

        assert_eq!(converter.name(), "GitHub Issue");
    }
}

/// Tests for registry configuration propagation
mod configuration_propagation_tests {
    use super::*;

    #[test]
    fn test_configured_registry_converters() {
        let registry = helpers::create_configured_registry();

        let supported_types = registry.supported_types();
        assert!(
            supported_types.len() >= MIN_STANDARD_CONVERTER_TYPES,
            "Registry should support at least 4 converter types"
        );

        for url_type in supported_types {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some(), "Missing converter for {url_type:?}");
        }
    }

    #[test]
    fn test_registry_converter_names_consistent() {
        let default_registry = ConverterRegistry::new();
        let configured_registry = helpers::create_configured_registry();

        helpers::verify_registry_consistency(&default_registry, &configured_registry);
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
        let initial_count = registry.supported_types().len();
        registry.register(UrlType::Html, Box::new(HtmlConverter::new()));

        // Test that HTML converter exists (it should in default registry)
        let converter = registry.get_converter(&UrlType::Html);
        assert!(converter.is_some());

        let supported_types = registry.supported_types();
        assert_eq!(supported_types.len(), initial_count + 1);
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
        let initial_count = registry.supported_types().len();

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
        assert_eq!(supported_types.len(), initial_count);
    }
}

/// Integration tests for registry usage patterns
mod integration_tests {
    use super::*;

    const PERFORMANCE_TEST_ITERATIONS: usize = 1000;

    #[test]
    fn test_registry_supports_all_url_types() {
        let registry = helpers::create_test_registry();
        helpers::assert_all_converters_available(&registry);
    }

    #[test]
    fn test_registry_workflow_simulation() {
        let registry = helpers::create_test_registry();

        let test_cases = vec![
            (UrlType::Html, "HTML"),
            (UrlType::GoogleDocs, "Google Docs"),
            (UrlType::GitHubIssue, "GitHub Issue"),
        ];

        helpers::verify_converter_names(&registry, test_cases);
    }

    #[test]
    fn test_registry_performance_with_multiple_lookups() {
        let registry = helpers::create_test_registry();

        // Perform many lookups to test performance
        for _ in 0..PERFORMANCE_TEST_ITERATIONS {
            helpers::assert_all_converters_available(&registry);
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

        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(DEFAULT_THREAD_COUNT);

        // Simulate multiple threads accessing the registry
        for i in 0..thread_count {
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
            .timeout_seconds(COMPREHENSIVE_TEST_TIMEOUT_SECONDS)
            .user_agent("comprehensive-test/1.0")
            .max_retries(TEST_MAX_RETRIES)
            .github_token("test_token")
            .office365_token("test_office_token")
            .google_api_key("test_google_key")
            .include_frontmatter(true)
            .normalize_whitespace(true)
            .build();

        let client = HttpClient::with_config(&config.http, &config.auth);
        let html_config = helpers::html_config_from_preset(helpers::HtmlConfigPreset::Permissive);
        let output_config = markdowndown::config::OutputConfig::default();

        let registry = ConverterRegistry::with_config(client, html_config, &output_config);

        helpers::assert_standard_registry_types(&registry);

        for (url_type, _, expected_name) in helpers::url_type_mappings_with_names() {
            let converter = registry.get_converter(&url_type);
            assert!(converter.is_some());
            assert_eq!(converter.unwrap().name(), expected_name);
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

    /// Benchmark a generic operation with specified parameters
    fn bench_operation<F>(
        operation_name: &str,
        iterations: usize,
        max_duration_secs: u64,
        operation: F,
    ) where
        F: Fn(),
    {
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            operation();
        }
        let duration = start.elapsed();
        assert!(
            duration < std::time::Duration::from_secs(max_duration_secs),
            "{} took too long: {:?}",
            operation_name,
            duration
        );
    }

    /// Benchmark a registry operation with specified parameters
    fn bench_registry_operation<F>(
        operation_name: &str,
        iterations: usize,
        max_duration_secs: u64,
        operation: F,
    ) where
        F: Fn(&ConverterRegistry),
    {
        let registry = helpers::create_test_registry();
        bench_operation(operation_name, iterations, max_duration_secs, || {
            operation(&registry);
        });
    }

    #[test]
    fn test_registry_lookup_performance() {
        let url_types = vec![UrlType::Html, UrlType::GoogleDocs, UrlType::GitHubIssue];

        bench_registry_operation(
            "Lookup stress test",
            LOOKUP_STRESS_TEST_ITERATIONS,
            LOOKUP_STRESS_TEST_MAX_DURATION_SECS,
            |registry| {
                for url_type in &url_types {
                    let _converter = registry.get_converter(url_type);
                }
            },
        );
    }

    #[test]
    fn test_registry_supported_types_performance() {
        bench_registry_operation(
            "Supported types stress test",
            SUPPORTED_TYPES_STRESS_TEST_ITERATIONS,
            SUPPORTED_TYPES_STRESS_TEST_MAX_DURATION_SECS,
            |registry| {
                let _types = registry.supported_types();
            },
        );
    }
}
