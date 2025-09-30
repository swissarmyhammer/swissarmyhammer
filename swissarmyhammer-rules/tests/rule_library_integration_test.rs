//! Integration tests for RuleLibrary
//!
//! Tests the complete rule loading and filtering functionality

use std::collections::HashMap;
use swissarmyhammer_rules::{Rule, RuleFilter, RuleLibrary, RuleSource, Severity};

#[test]
fn test_rule_library_basic_operations() {
    let mut library = RuleLibrary::new();

    // Add some test rules
    let rule1 = Rule::new(
        "test-rule-1".to_string(),
        "Check for issue 1".to_string(),
        Severity::Error,
    );
    let rule2 = Rule::new(
        "test-rule-2".to_string(),
        "Check for issue 2".to_string(),
        Severity::Warning,
    );

    library.add(rule1).unwrap();
    library.add(rule2).unwrap();

    // Test list
    let rules = library.list().unwrap();
    assert_eq!(rules.len(), 2);

    // Test get
    let retrieved = library.get("test-rule-1").unwrap();
    assert_eq!(retrieved.name, "test-rule-1");
    assert_eq!(retrieved.severity, Severity::Error);

    // Test list_names
    let names = library.list_names().unwrap();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"test-rule-1".to_string()));
    assert!(names.contains(&"test-rule-2".to_string()));

    // Test search
    let results = library.search("test-rule").unwrap();
    assert_eq!(results.len(), 2);

    let results = library.search("rule-1").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "test-rule-1");

    // Test remove
    assert!(library.remove("test-rule-1").unwrap());
    assert_eq!(library.list().unwrap().len(), 1);
    assert!(!library.remove("test-rule-1").unwrap());
}

#[test]
fn test_rule_library_with_categories_and_tags() {
    let mut library = RuleLibrary::new();

    let rule1 = Rule::builder(
        "security-rule".to_string(),
        "Check security".to_string(),
        Severity::Error,
    )
    .category("security".to_string())
    .tag("critical".to_string())
    .tag("security".to_string())
    .build();

    let rule2 = Rule::builder(
        "quality-rule".to_string(),
        "Check quality".to_string(),
        Severity::Warning,
    )
    .category("code-quality".to_string())
    .tag("maintainability".to_string())
    .build();

    library.add(rule1).unwrap();
    library.add(rule2).unwrap();

    let rules = library.list().unwrap();
    assert_eq!(rules.len(), 2);

    // Find security rules
    let security_rules: Vec<_> = rules
        .iter()
        .filter(|r| r.category == Some("security".to_string()))
        .collect();
    assert_eq!(security_rules.len(), 1);
    assert_eq!(security_rules[0].name, "security-rule");
}

#[test]
fn test_rule_library_filtering() {
    let mut library = RuleLibrary::new();

    let rule1 = Rule::builder(
        "error-rule".to_string(),
        "Template".to_string(),
        Severity::Error,
    )
    .category("security".to_string())
    .build();

    let rule2 = Rule::builder(
        "warning-rule".to_string(),
        "Template".to_string(),
        Severity::Warning,
    )
    .category("security".to_string())
    .build();

    let rule3 = Rule::builder(
        "info-rule".to_string(),
        "Template".to_string(),
        Severity::Info,
    )
    .category("code-quality".to_string())
    .build();

    library.add(rule1).unwrap();
    library.add(rule2).unwrap();
    library.add(rule3).unwrap();

    // Filter by severity
    let filter = RuleFilter::by_severities(vec![Severity::Error, Severity::Warning]);
    let sources = HashMap::new();
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 2);

    // Filter by category
    let filter = RuleFilter::by_category("security");
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 2);

    // Filter by combined criteria
    let filter = RuleFilter::by_category("security").with_severities(vec![Severity::Error]);
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "error-rule");
}

#[test]
fn test_rule_library_with_sources() {
    let mut library = RuleLibrary::new();

    library
        .add(Rule::new(
            "builtin-rule".to_string(),
            "T".to_string(),
            Severity::Error,
        ))
        .unwrap();
    library
        .add(Rule::new(
            "user-rule".to_string(),
            "T".to_string(),
            Severity::Error,
        ))
        .unwrap();
    library
        .add(Rule::new(
            "local-rule".to_string(),
            "T".to_string(),
            Severity::Error,
        ))
        .unwrap();

    let mut sources = HashMap::new();
    sources.insert("builtin-rule".to_string(), RuleSource::Builtin);
    sources.insert("user-rule".to_string(), RuleSource::User);
    sources.insert("local-rule".to_string(), RuleSource::Local);

    // Filter by source
    let filter = RuleFilter::by_sources(vec![RuleSource::Builtin]);
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "builtin-rule");

    // Filter multiple sources
    let filter = RuleFilter::by_sources(vec![RuleSource::User, RuleSource::Local]);
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_rule_library_partial_filtering() {
    let mut library = RuleLibrary::new();

    let normal_rule = Rule::new(
        "normal".to_string(),
        "Normal template".to_string(),
        Severity::Error,
    );

    let partial_rule = Rule::new(
        "partial".to_string(),
        "{% partial %}\nPartial content".to_string(),
        Severity::Info,
    );

    library.add(normal_rule).unwrap();
    library.add(partial_rule).unwrap();

    // Default filter excludes partials
    let filter = RuleFilter::new();
    let sources = HashMap::new();
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "normal");

    // Include partials
    let filter = RuleFilter::new().with_partials(true);
    let filtered = library.list_filtered(&filter, &sources).unwrap();
    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_rule_validation() {
    let mut library = RuleLibrary::new();

    let valid_rule = Rule::new(
        "valid".to_string(),
        "Template content".to_string(),
        Severity::Error,
    );
    assert!(valid_rule.validate().is_ok());
    library.add(valid_rule).unwrap();

    let invalid_name = Rule::new("".to_string(), "Content".to_string(), Severity::Error);
    assert!(invalid_name.validate().is_err());

    let invalid_template = Rule::new("name".to_string(), "".to_string(), Severity::Error);
    assert!(invalid_template.validate().is_err());
}

#[test]
fn test_rule_library_get_nonexistent() {
    let library = RuleLibrary::new();
    let result = library.get("nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_rule_library_empty() {
    let library = RuleLibrary::new();
    assert!(library.list().unwrap().is_empty());
    assert!(library.list_names().unwrap().is_empty());
    assert!(library.search("anything").unwrap().is_empty());
}
