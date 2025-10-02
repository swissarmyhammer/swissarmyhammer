//! Integration test for builtin rule partials
//!
//! Tests that the actual builtin rules with partials work correctly

use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_rules::{RuleLibrary, RuleLoader, RulePartialAdapter};
use swissarmyhammer_templating::Template;

/// Get the path to the builtin rules directory
fn builtin_rules_path() -> String {
    // Tests run from the workspace root
    "../builtin/rules".to_string()
}

#[test]
fn test_builtin_partials_exist() {
    let loader = RuleLoader::new();
    let rules = loader
        .load_directory(builtin_rules_path())
        .expect("Failed to load builtin rules");

    // Find partial rules
    let partials: Vec<_> = rules.iter().filter(|r| r.is_partial()).collect();

    // Should have at least the partials we created
    assert!(!partials.is_empty(), "No partials found in builtin rules");

    // Check for specific partials we created
    let partial_names: Vec<_> = partials.iter().map(|p| p.name.as_str()).collect();
    assert!(
        partial_names.contains(&"_partials/pass-response"),
        "pass-response partial not found. Found: {:?}",
        partial_names
    );
    assert!(
        partial_names.contains(&"_partials/no-display-secrets"),
        "no-display-secrets partial not found. Found: {:?}",
        partial_names
    );
    assert!(
        partial_names.contains(&"_partials/report-format"),
        "report-format partial not found. Found: {:?}",
        partial_names
    );
    assert!(
        partial_names.contains(&"_partials/code-block"),
        "code-block partial not found. Found: {:?}",
        partial_names
    );
}

#[test]
fn test_rule_with_partials_loads() {
    let loader = RuleLoader::new();
    let rules = loader
        .load_directory(builtin_rules_path())
        .expect("Failed to load builtin rules");

    // Find a rule that uses partials (we updated no-hardcoded-secrets)
    let rule = rules
        .iter()
        .find(|r| r.name == "security/no-hardcoded-secrets")
        .expect("no-hardcoded-secrets rule not found");

    // Check that it contains include statements for partials
    assert!(
        rule.template.contains("{% include"),
        "Rule should contain include statements for partials"
    );
    assert!(
        rule.template.contains("_partials/"),
        "Rule should reference partials from _partials directory"
    );
}

#[test]
fn test_render_rule_with_partials() {
    let loader = RuleLoader::new();
    let rules = loader
        .load_directory(builtin_rules_path())
        .expect("Failed to load builtin rules");

    // Create a rule library with all rules (including partials)
    let mut library = RuleLibrary::new();
    for rule in rules {
        library.add(rule).expect("Failed to add rule to library");
    }

    // Create partial adapter
    let adapter = RulePartialAdapter::new(Arc::new(library));

    // Find a rule that uses partials
    let rule = adapter
        .library()
        .get("security/no-hardcoded-secrets")
        .expect("no-hardcoded-secrets rule not found");

    // Render the rule template with partials
    let template =
        Template::with_partials(&rule.template, adapter).expect("Failed to create template");

    let mut data = HashMap::new();
    data.insert("language".to_string(), "Rust".to_string());
    data.insert(
        "target_content".to_string(),
        "let api_key = \"test\";".to_string(),
    );

    let rendered = template.render(&data).expect("Failed to render template");

    // Check that partial content was included
    assert!(
        rendered.contains("If no issues are found, respond with \"PASS\""),
        "Should include pass-response partial content. Got: {}",
        rendered
    );
    assert!(
        rendered.contains("DO NOT display the actual secret value"),
        "Should include no-display-secrets partial content. Got: {}",
        rendered
    );
    assert!(
        rendered.contains("Report the line number"),
        "Should include report-format partial content. Got: {}",
        rendered
    );
    assert!(
        rendered.contains("Code to analyze:"),
        "Should include code-block partial content. Got: {}",
        rendered
    );
}

#[test]
fn test_multiple_rules_share_partials() {
    let loader = RuleLoader::new();
    let rules = loader
        .load_directory(builtin_rules_path())
        .expect("Failed to load builtin rules");

    // Create a rule library
    let mut library = RuleLibrary::new();
    for rule in rules {
        library.add(rule).expect("Failed to add rule to library");
    }

    // Create partial adapter
    let adapter = RulePartialAdapter::new(Arc::new(library));

    // Get multiple rules that use partials
    let rule1 = adapter
        .library()
        .get("security/no-hardcoded-secrets")
        .expect("no-hardcoded-secrets rule not found");
    let rule2 = adapter
        .library()
        .get("security/no-plaintext-credentials")
        .expect("no-plaintext-credentials rule not found");

    // Both should reference partials
    assert!(rule1.template.contains("{% include \"_partials/"));
    assert!(rule2.template.contains("{% include \"_partials/"));

    // Both should be able to render with separate adapters using the same library
    let adapter1 = RulePartialAdapter::new(Arc::clone(adapter.library_arc()));
    let adapter2 = RulePartialAdapter::new(Arc::clone(adapter.library_arc()));

    let template1 = Template::with_partials(&rule1.template, adapter1)
        .expect("Failed to create template for rule1");
    let template2 = Template::with_partials(&rule2.template, adapter2)
        .expect("Failed to create template for rule2");

    let mut data = HashMap::new();
    data.insert("language".to_string(), "Rust".to_string());
    data.insert("target_content".to_string(), "test code".to_string());

    let rendered1 = template1.render(&data).expect("Failed to render rule1");
    let rendered2 = template2.render(&data).expect("Failed to render rule2");

    // Both should have the shared partial content
    assert!(rendered1.contains("If no issues are found, respond with \"PASS\""));
    assert!(rendered2.contains("If no issues are found, respond with \"PASS\""));
}

#[test]
fn test_partial_validation() {
    let loader = RuleLoader::new();
    let rules = loader
        .load_directory(builtin_rules_path())
        .expect("Failed to load builtin rules");

    // Find partial rules and validate them
    for rule in rules.iter().filter(|r| r.is_partial()) {
        // All partials should validate successfully
        assert!(
            rule.validate().is_ok(),
            "Partial '{}' failed validation: {:?}",
            rule.name,
            rule.validate().err()
        );

        // All partials should have the standard description
        assert_eq!(
            rule.description,
            Some("Partial template for reuse in other rules".to_string()),
            "Partial '{}' should have standard description",
            rule.name
        );
    }
}
