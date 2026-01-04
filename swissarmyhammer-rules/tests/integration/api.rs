//! Integration tests for swissarmyhammer-rules crate

use swissarmyhammer_rules::{RuleSource, Severity};

#[test]
fn test_public_api_exports() {
    // Verify Severity enum is accessible
    let _severity: Severity = Severity::Error;

    // Verify RuleSource enum is accessible
    let _source: RuleSource = RuleSource::Builtin;
}

#[test]
fn test_severity_usage() {
    // Test Display trait
    let error = Severity::Error;
    assert_eq!(error.to_string(), "error");

    let warning = Severity::Warning;
    assert_eq!(warning.to_string(), "warning");

    // Test FromStr trait
    let parsed: Severity = "info".parse().unwrap();
    assert_eq!(parsed, Severity::Info);
}

#[test]
fn test_rule_source_usage() {
    // Test enum variants
    let builtin = RuleSource::Builtin;
    let user = RuleSource::User;
    let local = RuleSource::Local;

    // Test equality
    assert_eq!(builtin, RuleSource::Builtin);
    assert_ne!(user, local);
}

#[test]
fn test_rule_source_from_file_source() {
    // Test conversion from FileSource to RuleSource
    let builtin = RuleSource::from(swissarmyhammer_common::FileSource::Builtin);
    assert_eq!(builtin, RuleSource::Builtin);

    let user = RuleSource::from(swissarmyhammer_common::FileSource::User);
    assert_eq!(user, RuleSource::User);

    let local = RuleSource::from(swissarmyhammer_common::FileSource::Local);
    assert_eq!(local, RuleSource::Local);

    let dynamic = RuleSource::from(swissarmyhammer_common::FileSource::Dynamic);
    assert_eq!(dynamic, RuleSource::User);
}
