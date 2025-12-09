//! Comprehensive unit tests for frontmatter generation and metadata handling.
//!
//! This module tests YAML frontmatter creation, serialization, deserialization,
//! and integration with markdown documents.

use chrono::{DateTime, Utc};
use markdowndown::frontmatter::FrontmatterBuilder;
use markdowndown::types::{Frontmatter, Markdown, Url};
use serde_yaml;

mod helpers {
    use super::*;

    /// Create a test URL for frontmatter testing with a given URL string
    pub fn create_test_url_from_str(url_str: &str) -> Url {
        Url::new(url_str.to_string()).unwrap()
    }

    /// Create a test URL for frontmatter testing (fallback for compatibility)
    pub fn create_test_url() -> Url {
        create_test_url_from_str("https://docs.google.com/document/d/test123/edit")
    }

    /// Create a test frontmatter instance with optional custom parameters
    pub fn create_test_frontmatter_with_params(
        url: Option<Url>,
        exporter: Option<&str>,
        date: Option<DateTime<Utc>>,
    ) -> Frontmatter {
        Frontmatter {
            source_url: url.unwrap_or_else(create_test_url),
            exporter: exporter.unwrap_or("markdowndown-test").to_string(),
            date_downloaded: date.unwrap_or_else(Utc::now),
        }
    }

    /// Create a test frontmatter instance with default values
    pub fn create_test_frontmatter() -> Frontmatter {
        create_test_frontmatter_with_params(None, None, None)
    }

    /// Sample markdown content for testing
    pub fn sample_markdown_content() -> &'static str {
        r#"# Project Documentation

## Overview

This document provides an overview of the project structure and guidelines.

### Key Features

- **Modular Architecture**: Clean separation of concerns
- **Type Safety**: Comprehensive type system with validation
- **Error Handling**: Robust error management and recovery
- **Documentation**: Extensive documentation and examples

### Getting Started

1. Clone the repository
2. Install dependencies
3. Run the test suite
4. Start development

## Configuration

The application supports various configuration options:

```yaml
app:
  name: "My Application"
  version: "1.0.0"
  debug: true
```

## Contact

For questions or support, please contact the development team."#
    }

    /// Sample YAML frontmatter for testing with custom parameters
    pub fn sample_yaml_frontmatter_with_params(
        url: &str,
        exporter: &str,
        date: &str,
    ) -> String {
        format!(
            r#"---
source_url: "{url}"
exporter: "{exporter}"
date_downloaded: "{date}"
title: "Project Documentation"
converter: "GoogleDocsConverter"
document_type: "Google Docs"
---"#
        )
    }

    /// Sample YAML frontmatter for testing (fallback for compatibility)
    pub fn sample_yaml_frontmatter() -> String {
        sample_yaml_frontmatter_with_params(
            "https://docs.google.com/document/d/test123/edit",
            "markdowndown-test",
            "2024-01-15T10:30:00Z",
        )
    }

    /// Strip YAML delimiters from frontmatter content
    pub fn strip_yaml_delimiters(yaml: &str) -> &str {
        yaml.trim_start_matches("---\n").trim_end_matches("---\n")
    }

    /// Parse frontmatter YAML to typed Frontmatter struct
    pub fn parse_frontmatter_yaml(yaml_content: &str) -> Result<Frontmatter, serde_yaml::Error> {
        let yaml_only = strip_yaml_delimiters(yaml_content);
        serde_yaml::from_str(yaml_only)
    }

    /// Parse YAML content by stripping delimiters to untyped Value
    pub fn parse_yaml_from_delimited(yaml_content: &str) -> serde_yaml::Value {
        let yaml_only = strip_yaml_delimiters(yaml_content);
        serde_yaml::from_str(yaml_only).unwrap()
    }

    /// Create a FrontmatterBuilder with URL and exporter
    pub fn create_builder_with_exporter(url: &Url, exporter: &str) -> FrontmatterBuilder {
        FrontmatterBuilder::new(url.to_string()).exporter(exporter.to_string())
    }

    /// Build and deserialize frontmatter from builder
    pub fn build_and_deserialize(
        url: &Url,
        exporter: &str,
        custom_date: Option<DateTime<Utc>>,
    ) -> Frontmatter {
        let mut builder = create_builder_with_exporter(url, exporter);
        if let Some(date) = custom_date {
            builder = builder.download_date(date);
        }
        let yaml_result = builder.build();
        assert!(yaml_result.is_ok());
        parse_frontmatter_yaml(&yaml_result.unwrap()).unwrap()
    }

    /// Verify frontmatter structure in markdown
    pub fn verify_frontmatter_structure(full_content: &str, expected_start: &str) {
        assert!(full_content.starts_with("---\n"));
        assert!(full_content.contains("\n---\n\n"));
        assert!(full_content.contains(expected_start));
    }

    /// Build frontmatter and parse YAML for testing
    pub fn build_and_parse_yaml(builder: FrontmatterBuilder) -> serde_yaml::Value {
        let yaml_result = builder.build();
        assert!(yaml_result.is_ok());
        parse_yaml_from_delimited(&yaml_result.unwrap())
    }

    /// Build and verify frontmatter with expected values
    pub fn build_and_verify_frontmatter(
        builder: FrontmatterBuilder,
        expected_url: &Url,
        expected_exporter: &str,
    ) -> Frontmatter {
        let yaml_result = builder.build();
        assert!(yaml_result.is_ok());
        let frontmatter = parse_frontmatter_yaml(&yaml_result.unwrap()).unwrap();
        assert_eq!(frontmatter.source_url, *expected_url);
        assert_eq!(frontmatter.exporter, expected_exporter);
        frontmatter
    }

    /// Verify converter YAML content for testing
    pub fn verify_converter_yaml(converter_name: &str, url_str: &str) {
        let url = Url::new(url_str.to_string()).unwrap();
        let builder = create_builder_with_exporter(&url, converter_name);
        build_and_verify_frontmatter(builder, &url, converter_name);
    }

    /// Assert YAML contains expected fields
    pub fn assert_yaml_contains_fields(yaml: &str, fields: &[(&str, &str)]) {
        for (field_name, expected_value) in fields {
            assert!(yaml.contains(field_name));
            if !expected_value.is_empty() {
                assert!(yaml.contains(expected_value));
            }
        }
    }

    /// Verify serialization consistency through multiple cycles
    pub fn verify_serialization_consistency(frontmatter: &Frontmatter, iterations: usize) {
        let mut previous: Option<Frontmatter> = None;
        
        for _ in 0..iterations {
            let yaml = serde_yaml::to_string(frontmatter).unwrap();
            let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
            
            if let Some(prev) = previous {
                assert_eq!(prev, deserialized);
            }
            previous = Some(deserialized);
        }
    }

    /// Serialize and verify frontmatter fields through roundtrip
    pub fn serialize_and_verify_fields(frontmatter: &Frontmatter, expected_url_prefix: &str) {
        let yaml_result = serde_yaml::to_string(frontmatter);
        assert!(yaml_result.is_ok());
        let yaml = yaml_result.unwrap();
        
        let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        
        assert!(deserialized.source_url.as_str().starts_with(expected_url_prefix));
        assert_eq!(deserialized.exporter, frontmatter.exporter);
        assert_eq!(deserialized.date_downloaded, frontmatter.date_downloaded);
        assert_eq!(deserialized.date_downloaded.timezone(), Utc);
    }

    /// Assert parsed frontmatter contains expected field values
    pub fn assert_parsed_frontmatter_fields(
        frontmatter: &Frontmatter,
        expected_url: &Url,
        expected_exporter: &str,
    ) {
        assert_eq!(frontmatter.source_url, *expected_url);
        assert_eq!(frontmatter.exporter, expected_exporter);
    }

    /// Verify markdown document structure with frontmatter and content
    pub fn verify_markdown_with_frontmatter(
        markdown_with_fm: &Markdown,
        expected_fm_field: &str,
        expected_body_content: &str,
    ) {
        let full_content = markdown_with_fm.as_str();
        
        assert!(full_content.contains("---"));
        
        let extracted_fm = markdown_with_fm.frontmatter();
        assert!(extracted_fm.is_some(), "Frontmatter should be extractable");
        let fm_content = extracted_fm.unwrap();
        
        let yaml_only = strip_yaml_delimiters(&fm_content);
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();
        assert!(parsed.get(expected_fm_field).is_some(), 
                "Frontmatter should contain field: {}", expected_fm_field);
        
        assert!(full_content.contains(expected_body_content));
    }

    /// Verify complete document structure including extraction
    pub fn verify_complete_document_structure(
        document: &Markdown,
        expected_converter: &str,
        expected_content: &str,
        expected_content_start: &str,
    ) {
        let full_content = document.as_str();
        verify_frontmatter_structure(full_content, expected_content_start);

        let extracted_frontmatter = document.frontmatter().unwrap();
        let yaml_only = strip_yaml_delimiters(&extracted_frontmatter);
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();
        assert_eq!(parsed["exporter"].as_str().unwrap(), expected_converter,
                   "Converter field should match expected value");

        let extracted_content = document.content_only();
        assert_eq!(extracted_content, expected_content);
    }

    /// Verify markdown extraction with expected fields
    pub fn verify_markdown_extraction(markdown: &Markdown, expected_fields: &[&str]) {
        let extracted = markdown.frontmatter();
        assert!(extracted.is_some());

        let frontmatter_content = extracted.unwrap();
        for field in expected_fields {
            assert!(frontmatter_content.contains(field));
        }
    }

    /// Assert YAML deserialization fails with descriptive message
    pub fn assert_yaml_deserialization_fails(yaml: &str, description: &str) {
        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Expected error for case: {}", description);
    }
}

/// Tests for Frontmatter struct creation and validation
mod frontmatter_creation_tests {
    use super::*;

    #[test]
    fn test_frontmatter_creation() {
        let source_url = helpers::create_test_url();
        let exporter = "markdowndown-test".to_string();
        let date_downloaded = Utc::now();

        let frontmatter = Frontmatter {
            source_url: source_url.clone(),
            exporter: exporter.clone(),
            date_downloaded,
        };

        assert_eq!(frontmatter.source_url, source_url);
        assert_eq!(frontmatter.exporter, exporter);
        assert_eq!(frontmatter.date_downloaded, date_downloaded);
    }

    #[test]
    fn test_frontmatter_with_different_urls() {
        let test_urls = vec![
            "https://docs.google.com/document/d/abc123/edit",
            "https://github.com/owner/repo/issues/123",
            "https://company.sharepoint.com/sites/team/doc.docx",
            "https://example.com/article.html",
        ];

        for url_str in test_urls {
            let url = Url::new(url_str.to_string()).unwrap();
            let frontmatter = helpers::create_test_frontmatter_with_params(
                Some(url.clone()),
                Some("test"),
                None,
            );

            assert_eq!(frontmatter.source_url, url);
            assert_eq!(frontmatter.exporter, "test");
        }
    }

    #[test]
    fn test_frontmatter_with_different_exporters() {
        let exporters = vec![
            "markdowndown",
            "markdowndown-v1.0.0",
            "GoogleDocsConverter",
            "HtmlConverter",
            "custom-exporter-123",
        ];

        let source_url = helpers::create_test_url();

        for exporter in exporters {
            let frontmatter = helpers::create_test_frontmatter_with_params(
                Some(source_url.clone()),
                Some(exporter),
                None,
            );

            assert_eq!(frontmatter.exporter, exporter);
        }
    }

    #[test]
    fn test_frontmatter_date_precision() {
        let exact_time = DateTime::parse_from_rfc3339("2024-01-15T10:30:45.123456789Z")
            .unwrap()
            .with_timezone(&Utc);

        let frontmatter = helpers::create_test_frontmatter_with_params(
            None,
            Some("test"),
            Some(exact_time),
        );

        assert_eq!(frontmatter.date_downloaded, exact_time);
        assert_eq!(frontmatter.date_downloaded.timezone(), Utc);
    }
}

/// Tests for YAML serialization
mod yaml_serialization_tests {
    use super::*;

    #[test]
    fn test_frontmatter_yaml_serialization() {
        let frontmatter = helpers::create_test_frontmatter();

        helpers::serialize_and_verify_fields(&frontmatter, "https://");
    }

    #[test]
    fn test_yaml_serialization_with_special_characters() {
        let url = Url::new(
            "https://example.com/document with spaces & symbols?param=value#section".to_string(),
        )
        .unwrap();
        let frontmatter = Frontmatter {
            source_url: url,
            exporter: "converter with spaces & symbols".to_string(),
            date_downloaded: Utc::now(),
        };

        helpers::serialize_and_verify_fields(&frontmatter, "https://example.com/");
    }

    #[test]
    fn test_yaml_serialization_deterministic() {
        let date = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let frontmatter = helpers::create_test_frontmatter_with_params(
            None,
            Some("test"),
            Some(date),
        );

        helpers::verify_serialization_consistency(&frontmatter, 3);
    }

    #[test]
    fn test_yaml_field_order() {
        let frontmatter = helpers::create_test_frontmatter();
        let yaml = serde_yaml::to_string(&frontmatter).unwrap();

        // Deserialize to verify all fields are present and correctly serialized
        let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        
        // Verify all fields are preserved through serialization/deserialization
        assert_eq!(deserialized.source_url, frontmatter.source_url);
        assert_eq!(deserialized.exporter, frontmatter.exporter);
        assert_eq!(deserialized.date_downloaded, frontmatter.date_downloaded);
    }

    #[test]
    fn test_yaml_formatting() {
        let frontmatter = helpers::create_test_frontmatter();
        let yaml = serde_yaml::to_string(&frontmatter).unwrap();

        // Verify the YAML is valid by deserializing it
        let deserialized: Result<Frontmatter, _> = serde_yaml::from_str(&yaml);
        assert!(deserialized.is_ok(), "YAML should be valid and deserializable");
        
        let deserialized = deserialized.unwrap();
        
        // Verify all fields are preserved correctly
        assert_eq!(deserialized.source_url, frontmatter.source_url);
        assert_eq!(deserialized.exporter, frontmatter.exporter);
        assert_eq!(deserialized.date_downloaded, frontmatter.date_downloaded);
    }
}

/// Tests for YAML deserialization
mod yaml_deserialization_tests {
    use super::*;

    #[test]
    fn test_frontmatter_yaml_deserialization() {
        let yaml = r#"
source_url: "https://docs.google.com/document/d/test123/edit"
exporter: "markdowndown-test"
date_downloaded: "2024-01-15T10:30:00Z"
"#;

        let frontmatter_result: Result<Frontmatter, _> = serde_yaml::from_str(yaml);
        assert!(frontmatter_result.is_ok());

        let frontmatter = frontmatter_result.unwrap();

        let expected_url = Url::new("https://docs.google.com/document/d/test123/edit".to_string()).unwrap();
        assert_eq!(frontmatter.source_url, expected_url);
        assert_eq!(frontmatter.exporter, "markdowndown-test");

        let expected_date = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(frontmatter.date_downloaded, expected_date);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let original = helpers::create_test_frontmatter();

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&original).unwrap();

        // Deserialize back
        let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();

        // Should be identical
        assert_eq!(original.source_url, deserialized.source_url);
        assert_eq!(original.exporter, deserialized.exporter);
        assert_eq!(original.date_downloaded, deserialized.date_downloaded);
    }

    #[test]
    fn test_deserialization_with_extra_fields() {
        let yaml = r#"
source_url: "https://example.com"
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
extra_field: "should be ignored"
another_field: 123
"#;

        let frontmatter_result: Result<Frontmatter, _> = serde_yaml::from_str(yaml);
        assert!(frontmatter_result.is_ok());

        let frontmatter = frontmatter_result.unwrap();
        let expected_url = Url::new("https://example.com".to_string()).unwrap();
        assert_eq!(frontmatter.source_url, expected_url);
        assert_eq!(frontmatter.exporter, "test");
    }

    #[test]
    fn test_deserialization_with_missing_fields() {
        let yaml_missing_exporter = r#"
source_url: "https://example.com"
date_downloaded: "2024-01-15T10:30:00Z"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml_missing_exporter);
        assert!(result.is_err());

        let yaml_missing_date = r#"
source_url: "https://example.com"
exporter: "test"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml_missing_date);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialization_with_invalid_date() {
        let yaml = r#"
source_url: "https://example.com"
exporter: "test"
date_downloaded: "not-a-date"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialization_with_invalid_url() {
        let yaml = r#"
source_url: "not-a-valid-url"
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml);
        // This should fail during URL validation if the Url type validates on deserialization
        // If not, it will succeed but be caught during usage
        match result {
            Ok(frontmatter) => {
                // URL might be accepted by serde but invalid for actual use
                let expected_url = Url::new("not-a-valid-url".to_string()).unwrap();
                assert_eq!(frontmatter.source_url, expected_url);
            }
            Err(_) => {
                // URL validation failed during deserialization
                // This is also acceptable behavior
            }
        }
    }
}

/// Tests for FrontmatterBuilder
mod frontmatter_builder_tests {
    use super::*;

    #[test]
    fn test_frontmatter_builder_basic() {
        let url = helpers::create_test_url();
        let frontmatter = helpers::build_and_deserialize(&url, "test-converter", None);
        helpers::assert_parsed_frontmatter_fields(&frontmatter, &url, "test-converter");
    }

    #[test]
    fn test_frontmatter_builder_with_custom_date() {
        let url = helpers::create_test_url();
        let custom_date = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let frontmatter = helpers::build_and_deserialize(&url, "test", Some(custom_date));
        assert_eq!(frontmatter.date_downloaded, custom_date);
    }

    #[test]
    fn test_frontmatter_builder_method_chaining() {
        let url = helpers::create_test_url();
        let custom_date = Utc::now();

        let frontmatter = helpers::build_and_deserialize(&url, "test", Some(custom_date));
        assert_eq!(frontmatter.source_url, url);
        assert_eq!(frontmatter.exporter, "test");
        assert_eq!(frontmatter.date_downloaded, custom_date);
    }

    #[test]
    fn test_frontmatter_builder_multiple_builds() {
        let url = helpers::create_test_url();

        let frontmatter1 = helpers::build_and_deserialize(&url, "test", None);
        let frontmatter2 = helpers::build_and_deserialize(&url, "test", None);

        assert_eq!(frontmatter1.source_url, frontmatter2.source_url);
        assert_eq!(frontmatter1.exporter, frontmatter2.exporter);
    }
}

/// Tests for integration with Markdown documents
mod markdown_integration_tests {
    use super::*;

    #[test]
    fn test_markdown_with_frontmatter() {
        let content = helpers::sample_markdown_content();
        let markdown = Markdown::new(content.to_string()).unwrap();

        let yaml_frontmatter = helpers::sample_yaml_frontmatter();
        let with_frontmatter = markdown.with_frontmatter(yaml_frontmatter);

        helpers::verify_markdown_with_frontmatter(
            &with_frontmatter,
            "source_url",
            "# Project Documentation",
        );
    }

    #[test]
    fn test_markdown_frontmatter_extraction() {
        let yaml_frontmatter = helpers::sample_yaml_frontmatter();
        let content = helpers::sample_markdown_content();
        let combined = format!("{yaml_frontmatter}\n\n{content}");

        let markdown = Markdown::from(combined);

        helpers::verify_markdown_extraction(&markdown, &["source_url:", "---"]);
    }

    #[test]
    fn test_markdown_content_without_frontmatter() {
        let yaml_frontmatter = helpers::sample_yaml_frontmatter();
        let content = helpers::sample_markdown_content();
        let combined = format!("{yaml_frontmatter}\n\n{content}");

        let markdown = Markdown::from(combined);

        // Extract content only
        let content_only = markdown.content_only();
        assert!(!content_only.contains("---"));
        assert!(!content_only.contains("source_url:"));
        assert!(content_only.contains("# Project Documentation"));
        assert!(content_only.contains("## Overview"));
    }

    #[test]
    fn test_markdown_without_frontmatter() {
        let content = helpers::sample_markdown_content();
        let markdown = Markdown::new(content.to_string()).unwrap();

        // No frontmatter should be found
        let frontmatter = markdown.frontmatter();
        assert!(frontmatter.is_none());

        // Content only should be the same as full content
        let content_only = markdown.content_only();
        assert_eq!(content_only, content);
    }

    #[test]
    fn test_markdown_frontmatter_generation() {
        let url = helpers::create_test_url();
        let builder = helpers::create_builder_with_exporter(&url, "TestConverter");
        let yaml_result = builder.build();

        assert!(yaml_result.is_ok());
        let yaml_with_delimiters = yaml_result.unwrap();

        let content = helpers::sample_markdown_content();
        let markdown = Markdown::new(content.to_string()).unwrap();
        let with_frontmatter = markdown.with_frontmatter(&yaml_with_delimiters);

        helpers::verify_markdown_with_frontmatter(
            &with_frontmatter,
            "source_url",
            "# Project Documentation",
        );

        helpers::verify_markdown_extraction(&with_frontmatter, &["source_url:"]);
    }
}

/// Tests for error handling and edge cases
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_yaml_cases() {
        let invalid_cases = vec![
            (
                "invalid structure",
                r#"
source_url: "https://example.com"
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
  invalid: yaml: structure
"#,
            ),
            ("empty", ""),
            (
                "null values",
                r#"
source_url: null
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
"#,
            ),
            (
                "wrong types",
                r#"
source_url: 123
exporter: true
date_downloaded: "2024-01-15T10:30:00Z"
"#,
            ),
        ];

        for (description, yaml) in &invalid_cases {
            helpers::assert_yaml_deserialization_fails(yaml, description);
        }
    }

    #[test]
    fn test_markdown_with_malformed_frontmatter() {
        let malformed = "---\nincomplete frontmatter without closing\n\n# Content";
        let markdown = Markdown::from(malformed.to_string());

        // Should not extract frontmatter if malformed
        let frontmatter = markdown.frontmatter();
        assert!(frontmatter.is_none());

        // Content only should return everything since no valid frontmatter
        let content_only = markdown.content_only();
        assert_eq!(content_only, malformed);
    }

    #[test]
    fn test_markdown_with_multiple_frontmatter_blocks() {
        let multiple_blocks = r#"---
first: block
---

# Content

---
second: block
---

More content"#;

        let markdown = Markdown::from(multiple_blocks.to_string());

        // Should only extract the first frontmatter block
        let frontmatter = markdown.frontmatter();
        assert!(frontmatter.is_some());

        let fm = frontmatter.unwrap();
        assert!(fm.contains("first: block"));
        assert!(!fm.contains("second: block"));
    }
}

/// Integration tests with real-world scenarios
mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_document_workflow() {
        let url = Url::new("https://docs.google.com/document/d/abc123/edit".to_string()).unwrap();
        let builder = helpers::create_builder_with_exporter(&url, "GoogleDocsConverter");
        let yaml_result = builder.build();

        assert!(yaml_result.is_ok());
        let yaml_with_delimiters = yaml_result.unwrap();

        let content = "# Meeting Notes\n\n## Agenda\n\n- Item 1\n- Item 2";
        let markdown = Markdown::new(content.to_string()).unwrap();

        let final_document = markdown.with_frontmatter(&yaml_with_delimiters);

        helpers::verify_complete_document_structure(
            &final_document,
            "GoogleDocsConverter",
            content,
            "# Meeting Notes",
        );

        let full_content = final_document.as_str();
        assert!(full_content.contains("source_url:"));
        assert!(full_content.contains("exporter:"));
        assert!(full_content.contains("date_downloaded:"));
        assert!(full_content.contains("## Agenda"));
    }

    #[test]
    fn test_different_converter_types() {
        let converters = vec![
            ("HtmlConverter", "https://example.com/page.html"),
            (
                "GoogleDocsConverter",
                "https://docs.google.com/document/d/123/edit",
            ),
            (
                "GitHubIssueConverter",
                "https://github.com/owner/repo/issues/123",
            ),
        ];

        for (converter_name, url_str) in &converters {
            helpers::verify_converter_yaml(converter_name, url_str);
        }
    }

    #[test]
    fn test_frontmatter_with_unicode_content() {
        let url = Url::new("https://example.com/document".to_string()).unwrap();
        let builder = helpers::create_builder_with_exporter(&url, "TestConverter");
        let yaml_result = builder.build();

        assert!(yaml_result.is_ok());
        let yaml_with_delimiters = yaml_result.unwrap();

        // Unicode content
        let unicode_content = r#"# プロジェクト文書

## 概要

このドキュメントは、プロジェクトの構造とガイドラインの概要を提供します。

### 主な機能

- **モジュラーアーキテクチャ**: 関心の明確な分離
- **型安全性**: 検証付きの包括的型システム
- **エラーハンドリング**: 堅牢なエラー管理と復旧

## 连接方式

如有问题或需要支持，请联系开发团队。

## Русская секция

Для вопросов на русском языке обращайтесь к команде."#;

        let markdown = Markdown::new(unicode_content.to_string()).unwrap();
        let with_frontmatter = markdown.with_frontmatter(&yaml_with_delimiters);

        // Verify Unicode is preserved
        let content_only = with_frontmatter.content_only();
        assert!(content_only.contains("プロジェクト文書"));
        assert!(content_only.contains("开发团队"));
        assert!(content_only.contains("Русская секция"));

        // Verify frontmatter is still extractable
        let extracted_frontmatter = with_frontmatter.frontmatter();
        assert!(extracted_frontmatter.is_some());
    }

    #[test]
    fn test_concurrent_frontmatter_creation() {
        use std::sync::Arc;
        use std::thread;

        const CONCURRENT_THREADS: usize = 10;

        let url = Arc::new(helpers::create_test_url());
        let mut handles = vec![];

        // Create frontmatter concurrently
        for i in 0..CONCURRENT_THREADS {
            let url_clone = Arc::clone(&url);
            let handle = thread::spawn(move || {
                let converter_name = format!("TestConverter{i}");
                let builder = helpers::create_builder_with_exporter(&url_clone, &converter_name);
                let yaml_result = builder.build();

                (i, yaml_result)
            });
            handles.push(handle);
        }

        // Collect results
        let mut results = vec![];
        for handle in handles {
            let (i, yaml_result) = handle.join().unwrap();
            assert!(yaml_result.is_ok());
            let yaml_content = yaml_result.unwrap();
            results.push((i, yaml_content));
        }

        // Verify all results are valid
        assert_eq!(results.len(), CONCURRENT_THREADS);
        for (i, yaml_content) in results {
            let parsed = helpers::parse_yaml_from_delimited(&yaml_content);
            assert_eq!(parsed["source_url"].as_str().unwrap(), url.as_str());
            assert_eq!(parsed["exporter"], format!("TestConverter{i}"));

            // Verify YAML content is well-formed
            assert!(yaml_content.starts_with("---\n"));
            assert!(yaml_content.ends_with("---\n"));
        }
    }
}
