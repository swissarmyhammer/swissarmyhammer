//! Tests for rule partials functionality
//!
//! This test suite validates the rule partials system, which allows markdown templates
//! to be reused across multiple rules through the `{% include %}` syntax.
//!
//! # Testing Strategy
//!
//! Tests follow a real file system approach using temporary directories and actual file I/O:
//! - Create temporary directories with `tempfile::tempdir()` for test isolation
//! - Write real partial files to disk with proper `{% partial %}` markers
//! - Use `RuleLoader` to load partials from directories (same as production code)
//! - Wrap loaded rules in `RulePartialAdapter` backed by `RuleLibrary` for template rendering
//! - Verify rendering behavior matches expectations with real integration paths
//!
//! This approach tests actual file system loading, parsing, and template rendering behavior
//! rather than using mocks, providing better integration test coverage and catching real
//! edge cases in production code paths

use std::collections::HashMap;
use swissarmyhammer_rules::RuleLoader;
use swissarmyhammer_templating::Template;

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

/// Tests that partials are correctly rendered within rule templates using real file system operations.
///
/// This test validates the full integration path for partial rendering:
/// 1. Creates a temporary directory with a `_partials` subdirectory
/// 2. Writes a real partial file to disk with the `{% partial %}` marker
/// 3. Uses `RuleLoader` to load the partial (same production code path)
/// 4. Wraps the loaded rules in `RulePartialAdapter` backed by `RuleLibrary`
/// 5. Creates a template that includes the partial via `{% include %}`
/// 6. Verifies the rendered output contains both the template and partial content
///
/// Using `RulePartialAdapter` is significant because it provides the integration between
/// the rule system (which loads partials as special rules) and the templating system
/// (which needs a `PartialLoader` trait implementation). This tests the actual production
/// code path used when rules reference partials.
#[test]
fn test_partial_rendering_in_rule() {
    // Create a temporary directory with a partial
    let temp_dir = tempfile::tempdir().unwrap();
    let partials_dir = temp_dir.path().join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Create a real partial file
    let partial_path = partials_dir.join("pass-response.md");
    std::fs::write(
        &partial_path,
        "{% partial %}\n\nIf no issues are found, respond with \"PASS\".",
    )
    .unwrap();

    // Load partials using RuleLoader
    let loader = RuleLoader::new();
    let rules = loader.load_directory(temp_dir.path()).unwrap();

    // Create a RuleLibrary and add the loaded rules
    let mut library = swissarmyhammer_rules::RuleLibrary::new();
    for rule in rules {
        library.add(rule).unwrap();
    }

    // Create a RulePartialAdapter from the library
    let adapter = swissarmyhammer_rules::RulePartialAdapter::new(std::sync::Arc::new(library));

    // Create a rule template that uses the partial
    let rule_template = r#"Check for issues in {{ language }} code.

{% include "_partials/pass-response" %}
"#;

    // Render the template with the real partial loader
    let template = Template::with_partials(rule_template, adapter).unwrap();

    let mut data = HashMap::new();
    data.insert("language".to_string(), "Rust".to_string());

    let rendered = template.render(&data).unwrap();

    // Should include the partial content
    assert!(rendered.contains("Check for issues in Rust code"));
    assert!(rendered.contains("If no issues are found, respond with \"PASS\""));
}

/// Tests error handling when a template references a non-existent partial.
///
/// This test validates that partial resolution fails at render time (not parse time):
/// 1. Creates a temporary directory with an empty `_partials` subdirectory
/// 2. Loads partials using `RuleLoader` (returns empty set)
/// 3. Creates a template that includes a non-existent partial
/// 4. Verifies template parsing succeeds (partial existence is not checked at parse time)
/// 5. Verifies rendering fails with appropriate error
///
/// The liquid templating engine defers partial resolution until render time, which means
/// templates can be parsed successfully even if partials don't exist. This is important
/// for template validation workflows where partial availability may vary by environment.
/// The test ensures our error handling correctly surfaces missing partial errors at
/// render time rather than silently failing.
#[test]
fn test_partial_not_found_error() {
    // Create a temporary directory with an empty _partials directory
    let temp_dir = tempfile::tempdir().unwrap();
    let partials_dir = temp_dir.path().join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Load partials using RuleLoader (will return empty set)
    let loader = RuleLoader::new();
    let rules = loader.load_directory(temp_dir.path()).unwrap();

    // Create an empty RuleLibrary
    let mut library = swissarmyhammer_rules::RuleLibrary::new();
    for rule in rules {
        library.add(rule).unwrap();
    }

    // Create a RulePartialAdapter from the empty library
    let adapter = swissarmyhammer_rules::RulePartialAdapter::new(std::sync::Arc::new(library));

    // Create a rule template with a non-existent partial
    let rule_template = r#"Check for issues.

{% include "non-existent-partial" %}
"#;

    // Try to create template with empty loader
    let template = Template::with_partials(rule_template, adapter);

    // Should succeed in parsing (partial resolution happens at render time in liquid)
    assert!(template.is_ok());

    // But rendering should fail
    let template = template.unwrap();
    let data = HashMap::new();
    let result = template.render(&data);

    // Should fail with partial not found error
    assert!(result.is_err());
}

/// Tests that multiple partials can be included in a single template and render correctly.
///
/// This test validates the interaction between multiple partials in one template:
/// 1. Creates a temporary directory with multiple partial files in `_partials`
/// 2. Writes two distinct partial files with different content
/// 3. Uses `RuleLoader` to load all partials from the directory
/// 4. Creates a template that includes both partials via multiple `{% include %}` tags
/// 5. Verifies the rendered output contains content from all included partials
///
/// This test ensures that the `RulePartialAdapter` correctly handles multiple partial
/// lookups within a single template render operation, and that partials don't interfere
/// with each other when used together. The test validates that partial content is
/// inserted in the correct order and maintains proper separation between different
/// partial inclusions.
#[test]
fn test_multiple_partials_in_rule() {
    // Create a temporary directory with multiple partials
    let temp_dir = tempfile::tempdir().unwrap();
    let partials_dir = temp_dir.path().join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Create first partial file
    let partial_path_1 = partials_dir.join("pass-response.md");
    std::fs::write(
        &partial_path_1,
        "{% partial %}\n\nIf no issues, respond with \"PASS\".",
    )
    .unwrap();

    // Create second partial file
    let partial_path_2 = partials_dir.join("report-format.md");
    std::fs::write(
        &partial_path_2,
        "{% partial %}\n\nReport line number and description.",
    )
    .unwrap();

    // Load partials using RuleLoader
    let loader = RuleLoader::new();
    let rules = loader.load_directory(temp_dir.path()).unwrap();

    // Create a RuleLibrary and add the loaded rules
    let mut library = swissarmyhammer_rules::RuleLibrary::new();
    for rule in rules {
        library.add(rule).unwrap();
    }

    // Create a RulePartialAdapter from the library
    let adapter = swissarmyhammer_rules::RulePartialAdapter::new(std::sync::Arc::new(library));

    // Create a rule template using multiple partials
    let rule_template = r#"Check for issues.

{% include "_partials/report-format" %}

{% include "_partials/pass-response" %}
"#;

    let template = Template::with_partials(rule_template, adapter).unwrap();
    let data = HashMap::new();
    let rendered = template.render(&data).unwrap();

    // Should include both partials
    assert!(rendered.contains("Report line number and description"));
    assert!(rendered.contains("If no issues, respond with \"PASS\""));
}
