//! Tests for rule partials functionality
//!
//! Tests that partials can be loaded and used within rules

use std::collections::HashMap;
use swissarmyhammer_rules::RuleLoader;
use swissarmyhammer_templating::{PartialLoader, Template};

#[test]
fn test_partial_marker_detection() {
    let partial_content = "{% partial %}\nIf no issues found, respond with \"PASS\".";
    let loader = RuleLoader::new();
    let partial = loader
        .load_from_string("test-partial", partial_content)
        .unwrap();

    assert!(partial.is_partial());
    assert_eq!(
        partial.description,
        Some("Partial template for reuse in other rules".to_string())
    );
}

#[test]
fn test_non_partial_rule() {
    let rule_content = r#"---
title: Test Rule
severity: error
---

Check for issues in {{ language }} code."#;

    let loader = RuleLoader::new();
    let rule = loader.load_from_string("test-rule", rule_content).unwrap();

    assert!(!rule.is_partial());
}

#[test]
fn test_partial_without_content_fails_validation() {
    let partial_content = "{% partial %}";
    let loader = RuleLoader::new();
    let partial = loader
        .load_from_string("empty-partial", partial_content)
        .unwrap();

    let result = partial.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must have content after"));
}

#[test]
fn test_load_partials_from_directory() {
    // Create a temporary directory structure
    let temp_dir = tempfile::tempdir().unwrap();
    let partials_dir = temp_dir.path().join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Create a partial file
    let partial_path = partials_dir.join("pass-response.md");
    std::fs::write(
        &partial_path,
        "{% partial %}\n\nIf no issues are found, respond with \"PASS\".",
    )
    .unwrap();

    // Load partials
    let loader = RuleLoader::new();
    let rules = loader.load_directory(temp_dir.path()).unwrap();

    // Should find the partial
    let partials: Vec<_> = rules.iter().filter(|r| r.is_partial()).collect();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].name, "_partials/pass-response");
}

#[test]
fn test_rule_using_partial_via_include() {
    // Create a temporary directory structure
    let temp_dir = tempfile::tempdir().unwrap();
    let partials_dir = temp_dir.path().join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Create a partial
    let partial_path = partials_dir.join("pass-response.md");
    std::fs::write(
        &partial_path,
        "{% partial %}\n\nIf no issues are found, respond with \"PASS\".",
    )
    .unwrap();

    // Create a rule that uses the partial
    let rule_path = temp_dir.path().join("test-rule.md");
    std::fs::write(
        &rule_path,
        r#"---
title: Test Rule
severity: error
---

Check for issues in {{ language }} code.

{% include "_partials/pass-response" %}
"#,
    )
    .unwrap();

    // Load rules
    let loader = RuleLoader::new();
    let rules = loader.load_directory(temp_dir.path()).unwrap();

    // Should find both the partial and the rule
    assert_eq!(rules.len(), 2);

    // Find the rule (not the partial)
    let rule = rules.iter().find(|r| r.name == "test-rule").unwrap();
    assert!(!rule.is_partial());
    assert!(rule.template.contains("{% include"));
}

#[test]
fn test_partial_rendering_in_rule() {
    // Create a mock partial loader
    struct MockPartialLoader;

    impl std::fmt::Debug for MockPartialLoader {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockPartialLoader").finish()
        }
    }

    impl PartialLoader for MockPartialLoader {
        fn contains(&self, name: &str) -> bool {
            name == "_partials/pass-response"
        }

        fn names(&self) -> Vec<String> {
            vec!["_partials/pass-response".to_string()]
        }

        fn try_get(&self, name: &str) -> Option<std::borrow::Cow<'_, str>> {
            if name == "_partials/pass-response" {
                Some(std::borrow::Cow::Borrowed(
                    "\n\nIf no issues are found, respond with \"PASS\".",
                ))
            } else {
                None
            }
        }
    }

    // Create a rule template that uses the partial
    let rule_template = r#"Check for issues in {{ language }} code.

{% include "_partials/pass-response" %}
"#;

    // Render the template with the partial loader
    let template = Template::with_partials(rule_template, MockPartialLoader).unwrap();

    let mut data = HashMap::new();
    data.insert("language".to_string(), "Rust".to_string());

    let rendered = template.render(&data).unwrap();

    // Should include the partial content
    assert!(rendered.contains("Check for issues in Rust code"));
    assert!(rendered.contains("If no issues are found, respond with \"PASS\""));
}

#[test]
fn test_partial_not_found_error() {
    // Create a rule template with a non-existent partial
    let rule_template = r#"Check for issues.

{% include "non-existent-partial" %}
"#;

    // Create an empty partial loader
    struct EmptyPartialLoader;

    impl std::fmt::Debug for EmptyPartialLoader {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("EmptyPartialLoader").finish()
        }
    }

    impl PartialLoader for EmptyPartialLoader {
        fn contains(&self, _name: &str) -> bool {
            false
        }

        fn names(&self) -> Vec<String> {
            Vec::new()
        }

        fn try_get(&self, _name: &str) -> Option<std::borrow::Cow<'_, str>> {
            None
        }
    }

    // Try to create template with empty loader
    let template = Template::with_partials(rule_template, EmptyPartialLoader);

    // Should succeed in parsing (partial resolution happens at render time in liquid)
    assert!(template.is_ok());

    // But rendering should fail
    let template = template.unwrap();
    let data = HashMap::new();
    let result = template.render(&data);

    // Should fail with partial not found error
    assert!(result.is_err());
}

#[test]
fn test_multiple_partials_in_rule() {
    // Create a mock partial loader with multiple partials
    struct MultiPartialLoader;

    impl std::fmt::Debug for MultiPartialLoader {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MultiPartialLoader").finish()
        }
    }

    impl PartialLoader for MultiPartialLoader {
        fn contains(&self, name: &str) -> bool {
            matches!(name, "_partials/pass-response" | "_partials/report-format")
        }

        fn names(&self) -> Vec<String> {
            vec![
                "_partials/pass-response".to_string(),
                "_partials/report-format".to_string(),
            ]
        }

        fn try_get(&self, name: &str) -> Option<std::borrow::Cow<'_, str>> {
            match name {
                "_partials/pass-response" => Some(std::borrow::Cow::Borrowed(
                    "If no issues, respond with \"PASS\".",
                )),
                "_partials/report-format" => Some(std::borrow::Cow::Borrowed(
                    "Report line number and description.",
                )),
                _ => None,
            }
        }
    }

    // Create a rule template using multiple partials
    let rule_template = r#"Check for issues.

{% include "_partials/report-format" %}

{% include "_partials/pass-response" %}
"#;

    let template = Template::with_partials(rule_template, MultiPartialLoader).unwrap();
    let data = HashMap::new();
    let rendered = template.render(&data).unwrap();

    // Should include both partials
    assert!(rendered.contains("Report line number and description"));
    assert!(rendered.contains("If no issues, respond with \"PASS\""));
}
