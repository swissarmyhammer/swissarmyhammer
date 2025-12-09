//! Property-based tests using proptest for robustness validation.

mod helpers;
mod unit {
    pub mod property_tests;
}

mod frontmatter_properties {
    use chrono::{DateTime, Utc};
    use markdowndown::frontmatter::FrontmatterBuilder;
    use markdowndown::types::{Frontmatter, Markdown, Url};
    use proptest::prelude::*;

    /// Strategy for generating valid HTTP/HTTPS URLs
    fn valid_url_strategy() -> impl Strategy<Value = String> {
        (
            prop::sample::select(vec!["http", "https"]),
            "[a-z0-9-]{1,20}",
            prop::sample::select(vec!["com", "org", "net", "edu", "gov", "io"]),
            prop::option::of("[a-z0-9/-]{0,30}"),
        )
            .prop_map(|(scheme, domain, tld, path)| match path {
                Some(p) if !p.is_empty() => format!("{scheme}://{domain}.{tld}/{p}"),
                _ => format!("{scheme}://{domain}.{tld}"),
            })
    }

    /// Strategy for generating exporter names
    fn exporter_name_strategy() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z][a-zA-Z0-9_-]{3,30}").unwrap()
    }

    /// Strategy for generating RFC3339 timestamps with various precisions
    fn timestamp_strategy() -> impl Strategy<Value = DateTime<Utc>> {
        (
            1609459200i64..1735689600i64, // 2021-01-01 to 2025-01-01
            0u32..1_000_000_000,          // nanoseconds
        )
            .prop_map(|(secs, nanos)| {
                DateTime::from_timestamp(secs, nanos)
                    .unwrap()
                    .with_timezone(&Utc)
            })
    }

    proptest! {
        #[test]
        fn test_frontmatter_with_arbitrary_urls(url_str in valid_url_strategy()) {
            let url = Url::new(url_str.clone()).unwrap();
            let frontmatter = Frontmatter {
                source_url: url.clone(),
                exporter: "test".to_string(),
                date_downloaded: Utc::now(),
            };

            prop_assert_eq!(frontmatter.source_url, url);
            prop_assert_eq!(frontmatter.exporter, "test");
        }

        #[test]
        fn test_frontmatter_with_arbitrary_exporters(
            exporter in exporter_name_strategy()
        ) {
            let url = Url::new("https://example.com".to_string()).unwrap();
            let frontmatter = Frontmatter {
                source_url: url,
                exporter: exporter.clone(),
                date_downloaded: Utc::now(),
            };

            prop_assert_eq!(frontmatter.exporter, exporter);
        }

        #[test]
        fn test_frontmatter_with_arbitrary_timestamps(
            timestamp in timestamp_strategy()
        ) {
            let url = Url::new("https://example.com".to_string()).unwrap();
            let frontmatter = Frontmatter {
                source_url: url,
                exporter: "test".to_string(),
                date_downloaded: timestamp,
            };

            prop_assert_eq!(frontmatter.date_downloaded, timestamp);
            prop_assert_eq!(frontmatter.date_downloaded.timezone(), Utc);
        }

        #[test]
        fn test_yaml_serialization_with_arbitrary_values(
            url_str in valid_url_strategy(),
            exporter in exporter_name_strategy(),
            timestamp in timestamp_strategy()
        ) {
            let url = Url::new(url_str).unwrap();
            let frontmatter = Frontmatter {
                source_url: url.clone(),
                exporter: exporter.clone(),
                date_downloaded: timestamp,
            };

            let yaml = serde_yaml::to_string(&frontmatter).unwrap();
            let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();

            prop_assert_eq!(deserialized.source_url, url);
            prop_assert_eq!(deserialized.exporter, exporter);
            prop_assert_eq!(deserialized.date_downloaded, timestamp);
        }

        #[test]
        fn test_builder_with_arbitrary_values(
            url_str in valid_url_strategy(),
            exporter in exporter_name_strategy(),
            timestamp in timestamp_strategy()
        ) {
            let builder = FrontmatterBuilder::new(url_str.clone())
                .exporter(exporter.clone())
                .download_date(timestamp);

            let yaml_result = builder.build();
            prop_assert!(yaml_result.is_ok());

            let yaml = yaml_result.unwrap();
            let yaml_only = yaml.trim_start_matches("---\n").trim_end_matches("---\n");
            let parsed: Frontmatter = serde_yaml::from_str(yaml_only).unwrap();

            prop_assert_eq!(parsed.source_url.as_str(), url_str);
            prop_assert_eq!(parsed.exporter, exporter);
            prop_assert_eq!(parsed.date_downloaded, timestamp);
        }

        #[test]
        fn test_concurrent_creation_with_arbitrary_values(
            url_str in valid_url_strategy(),
            thread_count in 2usize..20
        ) {
            use std::sync::Arc;
            use std::thread;

            let url_arc = Arc::new(url_str);
            let mut handles = vec![];

            for i in 0..thread_count {
                let url_clone = Arc::clone(&url_arc);
                let handle = thread::spawn(move || {
                    let exporter = format!("Converter{i}");
                    let builder = FrontmatterBuilder::new((*url_clone).clone())
                        .exporter(exporter.clone());
                    let yaml_result = builder.build();
                    (i, yaml_result, exporter)
                });
                handles.push(handle);
            }

            for handle in handles {
                let (_i, yaml_result, _exporter) = handle.join().unwrap();
                prop_assert!(yaml_result.is_ok());
            }
        }

        #[test]
        fn test_markdown_with_arbitrary_frontmatter(
            url_str in valid_url_strategy(),
            exporter in exporter_name_strategy(),
            timestamp_str in prop::string::string_regex("202[0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9]Z").unwrap()
        ) {
            let yaml_frontmatter = format!(
                r#"---
source_url: "{url_str}"
exporter: "{exporter}"
date_downloaded: "{timestamp_str}"
---"#
            );
            let content = "# Test Content\n\nTest body.";
            let markdown = Markdown::new(content.to_string()).unwrap();

            let with_frontmatter = markdown.with_frontmatter(&yaml_frontmatter);

            let extracted = with_frontmatter.frontmatter();
            prop_assert!(extracted.is_some());

            let fm_content = extracted.unwrap();
            prop_assert!(fm_content.contains(&url_str));
            prop_assert!(fm_content.contains(&exporter));
        }

        #[test]
        fn test_converter_types_with_arbitrary_urls(
            url_str in valid_url_strategy(),
            converter_name in exporter_name_strategy()
        ) {
            let builder = FrontmatterBuilder::new(url_str.clone())
                .exporter(converter_name.clone());

            let yaml_result = builder.build();
            prop_assert!(yaml_result.is_ok());

            let yaml = yaml_result.unwrap();
            let yaml_only = yaml.trim_start_matches("---\n").trim_end_matches("---\n");
            let frontmatter: Frontmatter = serde_yaml::from_str(yaml_only).unwrap();

            prop_assert_eq!(frontmatter.source_url.as_str(), url_str);
            prop_assert_eq!(frontmatter.exporter, converter_name);
        }
    }
}
