//! Integration tests for RuleSet loading and execution.
//!
//! These tests verify:
//! - RuleSets are loaded from directories with VALIDATOR.md
//! - Rules are discovered within RuleSets
//! - Inheritance works (severity, timeout)
//! - Precedence works (Project > User > Builtin)
//! - Matching works at RuleSet level

mod test_helpers;

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use std::fs;
use tempfile::TempDir;
use test_helpers::{
    create_test_ruleset, minimal_rule, minimal_ruleset_manifest, rule_with_severity,
    rule_with_timeout, ruleset_manifest_with_settings,
};

#[test]
fn test_load_builtin_rulesets() {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    // Should have loaded at least 4 RuleSets
    // (security-rules, command-safety, code-quality, test-integrity)
    assert!(
        loader.ruleset_count() >= 4,
        "Should have at least 4 builtin RuleSets, got {}",
        loader.ruleset_count()
    );

    // Check for expected RuleSets
    assert!(
        loader.get_ruleset("security-rules").is_some(),
        "Should have security-rules"
    );
    assert!(
        loader.get_ruleset("command-safety").is_some(),
        "Should have command-safety"
    );
    assert!(
        loader.get_ruleset("code-quality").is_some(),
        "Should have code-quality"
    );
    assert!(
        loader.get_ruleset("rust-conventions").is_none(),
        "rust-conventions should not be loaded (removed, replaced by dtolnay)"
    );
    assert!(
        loader.get_ruleset("test-integrity").is_some(),
        "Should have test-integrity"
    );
    assert!(
        loader.get_ruleset("session-lifecycle").is_none(),
        "session-lifecycle should not be loaded (removed)"
    );
}

#[test]
fn test_ruleset_contains_rules() {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    let security_rules = loader
        .get_ruleset("security-rules")
        .expect("security-rules should be loaded");

    // Should have multiple rules
    assert!(
        security_rules.rules.len() >= 2,
        "security-rules should have at least 2 rules"
    );

    // Check for specific rules
    let rule_names: Vec<&str> = security_rules
        .rules
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert!(
        rule_names.contains(&"no-secrets"),
        "Should have no-secrets rule"
    );
    assert!(
        rule_names.contains(&"input-validation"),
        "Should have input-validation rule"
    );
}

#[test]
fn test_ruleset_from_user_directory() {
    let temp = TempDir::new().unwrap();
    let validators_base = temp.path();

    // Create a user RuleSet
    let ruleset_dir = create_test_ruleset(validators_base, "my-custom-ruleset");

    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        minimal_ruleset_manifest("my-custom-ruleset", "My custom validation rules"),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("rule1.md"),
        minimal_rule("rule1", "First rule"),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("rule2.md"),
        minimal_rule("rule2", "Second rule"),
    )
    .unwrap();

    // Load the RuleSet
    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(&validators_base.join("validators"), ValidatorSource::User)
        .unwrap();

    // Verify RuleSet was loaded
    assert_eq!(loader.ruleset_count(), 1);
    let ruleset = loader.get_ruleset("my-custom-ruleset").unwrap();
    assert_eq!(ruleset.source, ValidatorSource::User);
    assert_eq!(ruleset.rules.len(), 2);

    // Verify rules
    let rule_names: Vec<&str> = ruleset.rules.iter().map(|r| r.name.as_str()).collect();
    assert!(rule_names.contains(&"rule1"));
    assert!(rule_names.contains(&"rule2"));
}

#[test]
fn test_rule_severity_override() {
    let temp = TempDir::new().unwrap();
    let ruleset_dir = create_test_ruleset(temp.path(), "test-ruleset");

    // Create manifest with default severity: warn
    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        ruleset_manifest_with_settings("test-ruleset", "Test RuleSet", "PostToolUse", "warn"),
    )
    .unwrap();

    // Create rules: one with default, one with override
    fs::write(
        ruleset_dir.join("rules").join("default-severity.md"),
        minimal_rule("default-severity", "Uses RuleSet default"),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("error-severity.md"),
        rule_with_severity("error-severity", "Overrides to error", "error"),
    )
    .unwrap();

    // Load and parse
    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(&temp.path().join("validators"), ValidatorSource::User)
        .unwrap();

    let ruleset = loader.get_ruleset("test-ruleset").unwrap();
    assert_eq!(ruleset.rules.len(), 2);

    // Find rules
    let default_rule = ruleset
        .rules
        .iter()
        .find(|r| r.name == "default-severity")
        .unwrap();
    let error_rule = ruleset
        .rules
        .iter()
        .find(|r| r.name == "error-severity")
        .unwrap();

    // Check effective severities
    use avp_common::validator::Severity;
    assert_eq!(
        default_rule.effective_severity(ruleset),
        Severity::Warn,
        "Should inherit RuleSet default"
    );
    assert_eq!(
        error_rule.effective_severity(ruleset),
        Severity::Error,
        "Should use override"
    );
}

#[test]
fn test_rule_timeout_override() {
    let temp = TempDir::new().unwrap();
    let ruleset_dir = create_test_ruleset(temp.path(), "timeout-test");

    // Create manifest with default timeout: 30
    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        minimal_ruleset_manifest("timeout-test", "Timeout test RuleSet"),
    )
    .unwrap();

    // Create rules: one with default, one with override
    fs::write(
        ruleset_dir.join("rules").join("default-timeout.md"),
        minimal_rule("default-timeout", "Uses RuleSet default"),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("long-timeout.md"),
        rule_with_timeout("long-timeout", "Overrides to 300", 300),
    )
    .unwrap();

    // Load and parse
    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(&temp.path().join("validators"), ValidatorSource::User)
        .unwrap();

    let ruleset = loader.get_ruleset("timeout-test").unwrap();

    // Find rules
    let default_rule = ruleset
        .rules
        .iter()
        .find(|r| r.name == "default-timeout")
        .unwrap();
    let long_rule = ruleset
        .rules
        .iter()
        .find(|r| r.name == "long-timeout")
        .unwrap();

    // Check effective timeouts
    assert_eq!(
        default_rule.effective_timeout(ruleset),
        30,
        "Should inherit RuleSet default (warn=30)"
    );
    assert_eq!(
        long_rule.effective_timeout(ruleset),
        300,
        "Should use override"
    );
}

#[test]
fn test_ruleset_precedence_user_overrides_builtin() {
    let temp = TempDir::new().unwrap();
    let validators_base = temp.path();

    // Create a user RuleSet with same name as builtin
    let ruleset_dir = create_test_ruleset(validators_base, "security-rules");

    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        ruleset_manifest_with_settings(
            "security-rules",
            "User security rules override",
            "PostToolUse",
            "info",
        ),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("custom-rule.md"),
        minimal_rule("custom-rule", "User custom rule"),
    )
    .unwrap();

    // Load builtins first, then user
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    assert!(
        loader.get_ruleset("security-rules").is_some(),
        "Builtin security-rules should exist"
    );
    assert_eq!(
        loader.get_ruleset("security-rules").unwrap().source,
        ValidatorSource::Builtin
    );

    // Load user (should override)
    loader
        .load_rulesets_directory(&validators_base.join("validators"), ValidatorSource::User)
        .unwrap();

    // Verify user version is active
    let ruleset = loader.get_ruleset("security-rules").unwrap();
    assert_eq!(ruleset.description(), "User security rules override");
    assert_eq!(ruleset.source, ValidatorSource::User);
    assert_eq!(ruleset.rules.len(), 1);
    assert_eq!(ruleset.rules[0].name, "custom-rule");
}

#[test]
fn test_ruleset_matching() {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    // Test matching for PostToolUse + Write + source file
    let ctx = avp_common::validator::MatchContext::from_json(
        avp_common::types::HookType::PostToolUse,
        &serde_json::json!({
            "tool_name": "Write",
            "tool_input": {"file_path": "test.rs"}
        }),
    );

    let matching = loader.matching_rulesets(&ctx);

    // Should match security-rules, code-quality, test-integrity
    // (rust-conventions removed, replaced by dtolnay project validator)
    let names: Vec<&str> = matching.iter().map(|rs| rs.name()).collect();

    assert!(
        names.contains(&"security-rules"),
        "security-rules should match PostToolUse + Write + .rs"
    );
    assert!(
        names.contains(&"code-quality"),
        "code-quality should match PostToolUse + Write + .rs"
    );
    assert!(
        !names.contains(&"rust-conventions"),
        "rust-conventions should not match (removed)"
    );
}

#[test]
fn test_ruleset_matching_bash_command() {
    let mut loader = ValidatorLoader::new();
    load_builtins(&mut loader);

    // Test matching for PreToolUse + Bash
    let ctx = avp_common::validator::MatchContext::from_json(
        avp_common::types::HookType::PreToolUse,
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"}
        }),
    );

    let matching = loader.matching_rulesets(&ctx);

    // Should match command-safety
    let names: Vec<&str> = matching.iter().map(|rs| rs.name()).collect();

    assert!(
        names.contains(&"command-safety"),
        "command-safety should match PreToolUse + Bash"
    );

    // Should NOT match security-rules (wrong trigger)
    assert!(
        !names.contains(&"security-rules"),
        "security-rules should not match PreToolUse"
    );
}
