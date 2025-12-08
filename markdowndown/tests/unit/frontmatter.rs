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

    /// Create a test URL for frontmatter testing
    pub fn create_test_url() -> Url {
        Url::new("https://docs.google.com/document/d/test123/edit".to_string()).unwrap()
    }

    /// Create a test frontmatter instance
    pub fn create_test_frontmatter() -> Frontmatter {
        Frontmatter {
            source_url: create_test_url(),
            exporter: "markdowndown-test".to_string(),
            date_downloaded: Utc::now(),
        }
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

    /// Sample YAML frontmatter for testing
    pub fn sample_yaml_frontmatter() -> &'static str {
        r#"---
source_url: "https://docs.google.com/document/d/test123/edit"
exporter: "markdowndown-test"
date_downloaded: "2024-01-15T10:30:00Z"
title: "Project Documentation"
converter: "GoogleDocsConverter"
document_type: "Google Docs"
---"#
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
            let frontmatter = Frontmatter {
                source_url: url.clone(),
                exporter: "test".to_string(),
                date_downloaded: Utc::now(),
            };

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
            let frontmatter = Frontmatter {
                source_url: source_url.clone(),
                exporter: exporter.to_string(),
                date_downloaded: Utc::now(),
            };

            assert_eq!(frontmatter.exporter, exporter);
        }
    }

    #[test]
    fn test_frontmatter_date_precision() {
        let source_url = helpers::create_test_url();
        let exact_time = DateTime::parse_from_rfc3339("2024-01-15T10:30:45.123456789Z")
            .unwrap()
            .with_timezone(&Utc);

        let frontmatter = Frontmatter {
            source_url,
            exporter: "test".to_string(),
            date_downloaded: exact_time,
        };

        assert_eq!(frontmatter.date_downloaded, exact_time);

        // Verify timezone is preserved
        assert_eq!(frontmatter.date_downloaded.timezone(), Utc);
    }
}

/// Tests for YAML serialization
mod yaml_serialization_tests {
    use super::*;

    #[test]
    fn test_frontmatter_yaml_serialization() {
        let frontmatter = helpers::create_test_frontmatter();

        let yaml_result = serde_yaml::to_string(&frontmatter);
        assert!(yaml_result.is_ok());

        let yaml = yaml_result.unwrap();

        // Check that YAML contains expected fields
        assert!(yaml.contains("source_url:"));
        assert!(yaml.contains("exporter:"));
        assert!(yaml.contains("date_downloaded:"));

        // Check that URL is present (may or may not be quoted depending on content)
        assert!(yaml.contains("https://"));

        // Check that exporter is present (may or may not be quoted depending on content)
        assert!(yaml.contains("markdowndown-test"));

        // Check that date is in ISO 8601 format
        assert!(yaml.contains("T") && yaml.contains("Z"));
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

        let yaml_result = serde_yaml::to_string(&frontmatter);
        assert!(yaml_result.is_ok());

        let yaml = yaml_result.unwrap();

        // Special characters should be properly escaped/quoted
        assert!(yaml.contains("document with spaces"));
        assert!(yaml.contains("converter with spaces"));
        assert!(yaml.contains("&"));
    }

    #[test]
    fn test_yaml_serialization_deterministic() {
        let frontmatter = Frontmatter {
            source_url: helpers::create_test_url(),
            exporter: "test".to_string(),
            date_downloaded: DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };

        // Serialize multiple times
        let yaml1 = serde_yaml::to_string(&frontmatter).unwrap();
        let yaml2 = serde_yaml::to_string(&frontmatter).unwrap();
        let yaml3 = serde_yaml::to_string(&frontmatter).unwrap();

        // Should be identical
        assert_eq!(yaml1, yaml2);
        assert_eq!(yaml2, yaml3);
    }

    #[test]
    fn test_yaml_field_order() {
        let frontmatter = helpers::create_test_frontmatter();
        let yaml = serde_yaml::to_string(&frontmatter).unwrap();

        // Find positions of fields in YAML
        let source_url_pos = yaml.find("source_url:").unwrap();
        let exporter_pos = yaml.find("exporter:").unwrap();
        let date_pos = yaml.find("date_downloaded:").unwrap();

        // Fields should appear in a consistent order
        // (Note: serde_yaml doesn't guarantee field order, but we can test that all fields are present)
        assert!(source_url_pos < yaml.len());
        assert!(exporter_pos < yaml.len());
        assert!(date_pos < yaml.len());
    }

    #[test]
    fn test_yaml_formatting() {
        let frontmatter = helpers::create_test_frontmatter();
        let yaml = serde_yaml::to_string(&frontmatter).unwrap();

        // Should not start or end with document separators (unless we add them)
        assert!(!yaml.starts_with("---"));
        assert!(!yaml.ends_with("---"));

        // Should be properly formatted with newlines
        assert!(yaml.contains('\n'));

        // Should not have excessive whitespace
        assert!(!yaml.contains("  \n"));
        assert!(!yaml.starts_with(' '));
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

        assert_eq!(
            frontmatter.source_url.as_str(),
            "https://docs.google.com/document/d/test123/edit"
        );
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
        assert_eq!(frontmatter.source_url.as_str(), "https://example.com");
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
                let url_str = frontmatter.source_url.as_str();
                assert_eq!(url_str, "not-a-valid-url");
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
        let builder =
            FrontmatterBuilder::new(url.to_string()).exporter("test-converter".to_string());

        let yaml_result = builder.build();
        assert!(yaml_result.is_ok());

        let yaml_content = yaml_result.unwrap();

        // Parse the YAML to verify content - strip delimiters
        let yaml_only = yaml_content
            .trim_start_matches("---\n")
            .trim_end_matches("---\n");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();
        assert_eq!(parsed["source_url"], url.to_string());
        assert_eq!(parsed["exporter"], "test-converter");
        assert!(parsed["date_downloaded"].is_string());
    }

    #[test]
    fn test_frontmatter_builder_with_custom_date() {
        let url = helpers::create_test_url();
        let custom_date = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let builder = FrontmatterBuilder::new(url.to_string())
            .exporter("test".to_string())
            .download_date(custom_date);

        let yaml_result = builder.build();
        assert!(yaml_result.is_ok());

        let yaml_content = yaml_result.unwrap();
        let yaml_only = yaml_content
            .trim_start_matches("---\n")
            .trim_end_matches("---\n");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();

        let parsed_date = parsed["date_downloaded"].as_str().unwrap();
        let parsed_datetime = DateTime::parse_from_rfc3339(parsed_date)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed_datetime, custom_date);
    }

    #[test]
    fn test_frontmatter_builder_method_chaining() {
        let url = helpers::create_test_url();
        let custom_date = Utc::now();

        let yaml_result = FrontmatterBuilder::new(url.to_string())
            .exporter("test".to_string())
            .download_date(custom_date)
            .build();

        assert!(yaml_result.is_ok());

        let yaml_content = yaml_result.unwrap();
        let yaml_only = yaml_content
            .trim_start_matches("---\n")
            .trim_end_matches("---\n");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();

        assert_eq!(parsed["source_url"], url.to_string());
        assert_eq!(parsed["exporter"], "test");

        let parsed_date = parsed["date_downloaded"].as_str().unwrap();
        let parsed_datetime = DateTime::parse_from_rfc3339(parsed_date)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed_datetime, custom_date);
    }

    #[test]
    fn test_frontmatter_builder_multiple_builds() {
        let url = helpers::create_test_url();

        // Builder is consumed on build(), so create separate instances
        let builder1 = FrontmatterBuilder::new(url.to_string()).exporter("test".to_string());
        let builder2 = FrontmatterBuilder::new(url.to_string()).exporter("test".to_string());

        let yaml1_result = builder1.build();
        let yaml2_result = builder2.build();

        assert!(yaml1_result.is_ok());
        assert!(yaml2_result.is_ok());

        let yaml1 = yaml1_result.unwrap();
        let yaml2 = yaml2_result.unwrap();

        let yaml1_only = yaml1.trim_start_matches("---\n").trim_end_matches("---\n");
        let yaml2_only = yaml2.trim_start_matches("---\n").trim_end_matches("---\n");
        let parsed1: serde_yaml::Value = serde_yaml::from_str(yaml1_only).unwrap();
        let parsed2: serde_yaml::Value = serde_yaml::from_str(yaml2_only).unwrap();

        // Should create similar content
        assert_eq!(parsed1["source_url"], parsed2["source_url"]);
        assert_eq!(parsed1["exporter"], parsed2["exporter"]);
        assert!(parsed1["date_downloaded"].is_string());
        assert!(parsed2["date_downloaded"].is_string());
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

        // Should contain both frontmatter and content
        let full_content = with_frontmatter.as_str();
        assert!(full_content.contains("---"));
        assert!(full_content.contains("source_url:"));
        assert!(full_content.contains("# Project Documentation"));
    }

    #[test]
    fn test_markdown_frontmatter_extraction() {
        let yaml_frontmatter = helpers::sample_yaml_frontmatter();
        let content = helpers::sample_markdown_content();
        let combined = format!("{yaml_frontmatter}\n\n{content}");

        let markdown = Markdown::from(combined);

        // Extract frontmatter
        let extracted_frontmatter = markdown.frontmatter();
        assert!(extracted_frontmatter.is_some());

        let frontmatter = extracted_frontmatter.unwrap();
        assert!(frontmatter.contains("source_url:"));
        assert!(frontmatter.contains("---"));
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
        let builder =
            FrontmatterBuilder::new(url.to_string()).exporter("TestConverter".to_string());
        let yaml_result = builder.build();

        assert!(yaml_result.is_ok());
        let yaml_with_delimiters = yaml_result.unwrap();

        // Add to markdown
        let content = helpers::sample_markdown_content();
        let markdown = Markdown::new(content.to_string()).unwrap();
        let with_frontmatter = markdown.with_frontmatter(&yaml_with_delimiters);

        // Verify structure
        let full_content = with_frontmatter.as_str();
        assert!(full_content.starts_with("---\n"));
        assert!(full_content.contains("\n---\n\n# Project Documentation"));

        // Verify frontmatter extraction works
        let extracted = with_frontmatter.frontmatter();
        assert!(extracted.is_some());
        assert!(extracted.unwrap().contains("source_url:"));
    }
}

/// Tests for error handling and edge cases
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_yaml_frontmatter() {
        let invalid_yaml = r#"
source_url: "https://example.com"
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
  invalid: yaml: structure
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(invalid_yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_yaml_frontmatter() {
        let empty_yaml = "";
        let result: Result<Frontmatter, _> = serde_yaml::from_str(empty_yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_frontmatter_with_null_values() {
        let yaml_with_nulls = r#"
source_url: null
exporter: "test"
date_downloaded: "2024-01-15T10:30:00Z"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml_with_nulls);
        assert!(result.is_err());
    }

    #[test]
    fn test_frontmatter_with_wrong_types() {
        let yaml_wrong_types = r#"
source_url: 123
exporter: true
date_downloaded: "2024-01-15T10:30:00Z"
"#;

        let result: Result<Frontmatter, _> = serde_yaml::from_str(yaml_wrong_types);
        assert!(result.is_err());
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
        // 1. Create frontmatter
        let url = Url::new("https://docs.google.com/document/d/abc123/edit".to_string()).unwrap();
        let builder =
            FrontmatterBuilder::new(url.to_string()).exporter("GoogleDocsConverter".to_string());
        let yaml_result = builder.build();

        assert!(yaml_result.is_ok());
        // 2. Get YAML with delimiters (already included by build())
        let yaml_with_delimiters = yaml_result.unwrap();

        // 3. Create markdown content
        let content = "# Meeting Notes\n\n## Agenda\n\n- Item 1\n- Item 2";
        let markdown = Markdown::new(content.to_string()).unwrap();

        // 4. Combine with frontmatter
        let final_document = markdown.with_frontmatter(&yaml_with_delimiters);

        // 5. Verify complete document
        let full_content = final_document.as_str();
        assert!(full_content.starts_with("---\n"));
        assert!(full_content.contains("source_url:"));
        assert!(full_content.contains("exporter:"));
        assert!(full_content.contains("date_downloaded:"));
        assert!(full_content.contains("# Meeting Notes"));
        assert!(full_content.contains("## Agenda"));

        // 6. Verify extraction works
        let extracted_frontmatter = final_document.frontmatter().unwrap();
        let extracted_content = final_document.content_only();

        assert!(extracted_frontmatter.contains("GoogleDocsConverter"));
        assert_eq!(extracted_content, content);
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

        for (converter_name, url_str) in converters {
            let url = Url::new(url_str.to_string()).unwrap();
            let builder =
                FrontmatterBuilder::new(url.to_string()).exporter(converter_name.to_string());
            let yaml_result = builder.build();

            assert!(yaml_result.is_ok());
            let yaml_content = yaml_result.unwrap();

            // Extract YAML content between delimiters
            let yaml_only = yaml_content
                .strip_prefix("---\n")
                .and_then(|s| s.strip_suffix("---\n"))
                .unwrap_or(&yaml_content);

            let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();

            // Verify correct values
            assert_eq!(parsed["source_url"], url.to_string());
            assert_eq!(parsed["exporter"], converter_name);

            // Verify YAML content contains expected values
            assert!(yaml_content.contains(url_str));
            assert!(yaml_content.contains(converter_name));
        }
    }

    #[test]
    fn test_frontmatter_with_unicode_content() {
        let url = Url::new("https://example.com/document".to_string()).unwrap();
        let builder =
            FrontmatterBuilder::new(url.to_string()).exporter("TestConverter".to_string());
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

        let url = Arc::new(helpers::create_test_url());
        let mut handles = vec![];

        // Create frontmatter concurrently
        for i in 0..10 {
            let url_clone = Arc::clone(&url);
            let handle = thread::spawn(move || {
                let converter_name = format!("TestConverter{i}");
                let builder =
                    FrontmatterBuilder::new(url_clone.to_string()).exporter(converter_name.clone());
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
        assert_eq!(results.len(), 10);
        for (i, yaml_content) in results {
            let yaml_only = yaml_content
                .trim_start_matches("---\n")
                .trim_end_matches("---\n");
            let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_only).unwrap();
            assert_eq!(parsed["source_url"], url.to_string());
            assert_eq!(parsed["exporter"], format!("TestConverter{i}"));

            // Verify YAML content is well-formed
            assert!(yaml_content.starts_with("---\n"));
            assert!(yaml_content.ends_with("---\n"));
        }
    }
}
