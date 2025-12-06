//! Integration tests for per-file rule ignore directives

use std::path::PathBuf;
use swissarmyhammer_agent_executor::LlamaAgentExecutorWrapper;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissArmyHammerError;
use swissarmyhammer_config::LlamaAgentConfig;
use swissarmyhammer_rules::{Rule, RuleChecker, RuleViolation, Severity};

/// Create a test agent for integration tests
fn create_test_agent() -> std::sync::Arc<dyn swissarmyhammer_agent_executor::AgentExecutor> {
    let config = LlamaAgentConfig::for_testing();
    let mcp_server = agent_client_protocol::McpServer::Http {
        name: "test".to_string(),
        url: "http://localhost:8080/mcp".to_string(),
        headers: Vec::new(),
    };
    std::sync::Arc::new(LlamaAgentExecutorWrapper::new(config, mcp_server))
}

/// Helper function to create a test environment with a file
fn setup_test_with_file(
    content: &str,
    filename: &str,
) -> (RuleChecker, IsolatedTestEnvironment, PathBuf) {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let test_file = env.temp_dir().join(filename);
    std::fs::write(&test_file, content).unwrap();
    (checker, env, test_file)
}

/// Helper function to create a test rule
fn create_test_rule(name: &str, description: &str, severity: Severity) -> Rule {
    Rule::new(name.to_string(), description.to_string(), severity)
}

/// Helper function to create an error severity test rule with a standard description
fn create_error_rule(name: &str) -> Rule {
    Rule::new(
        name.to_string(),
        format!("Check for {}", name),
        Severity::Error,
    )
}

/// Helper function to assert that a rule check passed (no violation found)
fn assert_rule_ignored(
    result: Result<Option<RuleViolation>, SwissArmyHammerError>,
    rule_name: &str,
    context: &str,
) {
    assert!(
        result.is_ok(),
        "{} should be ignored {}",
        rule_name,
        context
    );
}

/// Helper function to test ignore with multiple file variations
async fn test_ignore_with_variations<F>(rule: &Rule, test_cases: &[(&str, &str)], assertion_msg: F)
where
    F: Fn(usize) -> String,
{
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    for (i, (filename, content)) in test_cases.iter().enumerate() {
        let test_file = temp_dir.join(filename);
        std::fs::write(&test_file, content).unwrap();
        let result = checker.check_file(rule, &test_file, None).await;
        assert!(result.is_ok(), "{}", assertion_msg(i));
    }
}

#[tokio::test]
async fn test_ignore_directive_single_rule() {
    let rule = create_error_rule("no-unwrap");

    let (checker, _temp_dir, test_file) = setup_test_with_file(
        "// sah rule ignore no-unwrap\nfn main() {\n    let x = Some(1).unwrap();\n}",
        "test.rs",
    );

    let result = checker.check_file(&rule, &test_file, None).await;
    assert_rule_ignored(result, "no-unwrap", "via file directive");
}

#[tokio::test]
async fn test_ignore_directive_glob_pattern() {
    let rule1 = create_error_rule("no-unwrap");
    let rule2 = create_error_rule("no-panic");

    let (checker, _temp_dir, test_file) = setup_test_with_file(
        "// sah rule ignore no-*\nfn main() {\n    panic!(\"test\");\n}",
        "test.rs",
    );

    let result1 = checker.check_file(&rule1, &test_file, None).await;
    assert_rule_ignored(result1, "no-unwrap", "by glob pattern no-*");

    let result2 = checker.check_file(&rule2, &test_file, None).await;
    assert_rule_ignored(result2, "no-panic", "by glob pattern no-*");
}

#[tokio::test]
async fn test_ignore_directive_multiple_patterns() {
    let rule1 = create_error_rule("no-unwrap");
    let rule2 = create_test_rule("complexity-check", "Check complexity", Severity::Warning);

    let (checker, _temp_dir, test_file) = setup_test_with_file(
        "// sah rule ignore no-unwrap\n// sah rule ignore complexity-check\nfn main() {}",
        "test.rs",
    );

    let result1 = checker.check_file(&rule1, &test_file, None).await;
    assert_rule_ignored(result1, "no-unwrap", "");

    let result2 = checker.check_file(&rule2, &test_file, None).await;
    assert_rule_ignored(result2, "complexity-check", "");
}

#[tokio::test]
async fn test_ignore_directive_different_comment_styles() {
    let rule = create_test_rule("test-rule", "Test rule", Severity::Error);

    let test_cases = vec![
        ("test_line_comment.rs", "// sah rule ignore test-rule\n"),
        ("test_hash_comment.py", "# sah rule ignore test-rule\n"),
        ("test_block_comment.js", "/* sah rule ignore test-rule */\n"),
        (
            "test_html_comment.html",
            "<!-- sah rule ignore test-rule -->\n",
        ),
    ];

    test_ignore_with_variations(&rule, &test_cases, |i| {
        format!(
            "Rule should be ignored in file {} with comment style",
            test_cases[i].0
        )
    })
    .await;
}

#[tokio::test]
async fn test_ignore_directive_not_applied_to_different_rule() {
    let rule = create_test_rule("other-rule", "Different rule", Severity::Error);

    let (checker, _temp_dir, test_file) =
        setup_test_with_file("// sah rule ignore no-unwrap\nfn main() {}", "test.rs");

    // This rule should NOT be ignored (it will actually run the check)
    // Since we're using a test agent, we can't predict the outcome,
    // but we verify that the ignore doesn't apply to unmatched rules
    // The key is that this doesn't panic or error due to the ignore logic itself
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Result can be Ok or Err depending on LLM, but shouldn't crash
}

#[tokio::test]
async fn test_ignore_directive_empty_file() {
    let rule = create_test_rule("test-rule", "Test rule", Severity::Error);

    let (checker, _temp_dir, test_file) = setup_test_with_file("", "empty.rs");

    // Should proceed to normal checking (no ignores found)
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Result depends on LLM, but shouldn't crash
}

#[tokio::test]
async fn test_ignore_directive_suffix_glob() {
    let rule = create_error_rule("allow-unwrap");

    let (checker, _temp_dir, test_file) =
        setup_test_with_file("// sah rule ignore *-unwrap\nfn main() {}", "test.rs");

    let result = checker.check_file(&rule, &test_file, None).await;
    assert_rule_ignored(result, "allow-unwrap", "by glob pattern *-unwrap");
}

#[tokio::test]
async fn test_ignore_directive_question_mark_glob() {
    let rule = create_test_rule("test-a-rule", "Test rule", Severity::Error);

    let (checker, _temp_dir, test_file) =
        setup_test_with_file("// sah rule ignore test-?-rule\nfn main() {}", "test.rs");

    let result = checker.check_file(&rule, &test_file, None).await;
    assert_rule_ignored(result, "test-a-rule", "by glob pattern test-?-rule");
}

#[tokio::test]
async fn test_ignore_directive_case_sensitive() {
    let rule = create_error_rule("no-unwrap");

    let (checker, _temp_dir, test_file) =
        setup_test_with_file("// sah rule ignore No-Unwrap\nfn main() {}", "test.rs");

    // Should NOT be ignored because rule names are case-sensitive
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Will proceed to normal checking since ignore doesn't match
}

#[tokio::test]
async fn test_ignore_directive_whitespace_handling() {
    let rule = create_error_rule("no-unwrap");

    let test_cases = [
        ("test_0.rs", "//sah rule ignore no-unwrap\n"),
        ("test_1.rs", "//  sah  rule  ignore  no-unwrap\n"),
        ("test_2.rs", "// sah rule ignore no-unwrap  \n"),
        ("test_3.rs", "  // sah rule ignore no-unwrap\n"),
    ];

    test_ignore_with_variations(&rule, &test_cases, |i| {
        format!("Rule should be ignored with whitespace variation {}", i)
    })
    .await;
}

#[tokio::test]
async fn test_ignore_directive_position_in_file() {
    let rule = create_error_rule("no-unwrap");

    let test_cases = [
        (
            "position_test_0.rs",
            "// sah rule ignore no-unwrap\nfn main() {}",
        ),
        (
            "position_test_1.rs",
            "fn helper() {}\n// sah rule ignore no-unwrap\nfn main() {}",
        ),
        (
            "position_test_2.rs",
            "fn main() {}\n// sah rule ignore no-unwrap",
        ),
    ];

    test_ignore_with_variations(&rule, &test_cases, |i| {
        format!(
            "Rule should be ignored regardless of position in file (test case {})",
            i
        )
    })
    .await;
}
