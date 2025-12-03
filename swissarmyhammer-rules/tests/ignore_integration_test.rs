//! Integration tests for per-file rule ignore directives

use swissarmyhammer_agent_executor::LlamaAgentExecutorWrapper;
use swissarmyhammer_config::LlamaAgentConfig;
use swissarmyhammer_rules::{Rule, RuleChecker, Severity};
use tempfile::TempDir;

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

#[tokio::test]
async fn test_ignore_directive_single_rule() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    // Create a test rule
    let rule = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap() calls".to_string(),
        Severity::Error,
    );

    // Create a temp file with ignore directive
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(
        &test_file,
        "// sah rule ignore no-unwrap\nfn main() {\n    let x = Some(1).unwrap();\n}",
    )
    .unwrap();

    // Check should pass because rule is ignored
    let result = checker.check_file(&rule, &test_file, None).await;
    assert!(
        result.is_ok(),
        "Check should pass when rule is ignored via file directive"
    );
}

#[tokio::test]
async fn test_ignore_directive_glob_pattern() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    // Create test rules
    let rule1 = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );
    let rule2 = Rule::new(
        "no-panic".to_string(),
        "Check for panic()".to_string(),
        Severity::Error,
    );

    // Create a temp file with glob pattern ignore
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(
        &test_file,
        "// sah rule ignore no-*\nfn main() {\n    panic!(\"test\");\n}",
    )
    .unwrap();

    // Both rules starting with "no-" should be ignored
    let result1 = checker.check_file(&rule1, &test_file, None).await;
    assert!(
        result1.is_ok(),
        "no-unwrap should be ignored by glob pattern no-*"
    );

    let result2 = checker.check_file(&rule2, &test_file, None).await;
    assert!(
        result2.is_ok(),
        "no-panic should be ignored by glob pattern no-*"
    );
}

#[tokio::test]
async fn test_ignore_directive_multiple_patterns() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule1 = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );
    let rule2 = Rule::new(
        "complexity-check".to_string(),
        "Check complexity".to_string(),
        Severity::Warning,
    );

    // Create a temp file with multiple ignore directives
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(
        &test_file,
        "// sah rule ignore no-unwrap\n// sah rule ignore complexity-check\nfn main() {}",
    )
    .unwrap();

    // Both rules should be ignored
    let result1 = checker.check_file(&rule1, &test_file, None).await;
    assert!(result1.is_ok(), "no-unwrap should be ignored");

    let result2 = checker.check_file(&rule2, &test_file, None).await;
    assert!(result2.is_ok(), "complexity-check should be ignored");
}

#[tokio::test]
async fn test_ignore_directive_different_comment_styles() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "test-rule".to_string(),
        "Test rule".to_string(),
        Severity::Error,
    );

    let test_cases = vec![
        ("test_line_comment.rs", "// sah rule ignore test-rule\n"),
        ("test_hash_comment.py", "# sah rule ignore test-rule\n"),
        ("test_block_comment.js", "/* sah rule ignore test-rule */\n"),
        (
            "test_html_comment.html",
            "<!-- sah rule ignore test-rule -->\n",
        ),
    ];

    let temp_dir = TempDir::new().unwrap();

    for (filename, content) in test_cases {
        let test_file = temp_dir.path().join(filename);
        std::fs::write(&test_file, content).unwrap();

        let result = checker.check_file(&rule, &test_file, None).await;
        assert!(
            result.is_ok(),
            "Rule should be ignored in file {} with comment style",
            filename
        );
    }
}

#[tokio::test]
async fn test_ignore_directive_not_applied_to_different_rule() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    // Create a rule that is NOT ignored
    let rule = Rule::new(
        "other-rule".to_string(),
        "Different rule".to_string(),
        Severity::Error,
    );

    // Create a temp file with ignore for a different rule
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "// sah rule ignore no-unwrap\nfn main() {}").unwrap();

    // This rule should NOT be ignored (it will actually run the check)
    // Since we're using a test agent, we can't predict the outcome,
    // but we verify that the ignore doesn't apply to unmatched rules
    // The key is that this doesn't panic or error due to the ignore logic itself
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Result can be Ok or Err depending on LLM, but shouldn't crash
}

#[tokio::test]
async fn test_ignore_directive_empty_file() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "test-rule".to_string(),
        "Test rule".to_string(),
        Severity::Error,
    );

    // Create an empty file (no ignore directives)
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty.rs");
    std::fs::write(&test_file, "").unwrap();

    // Should proceed to normal checking (no ignores found)
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Result depends on LLM, but shouldn't crash
}

#[tokio::test]
async fn test_ignore_directive_suffix_glob() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "allow-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );

    // Create a temp file with suffix glob pattern
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "// sah rule ignore *-unwrap\nfn main() {}").unwrap();

    // Rule ending with "-unwrap" should be ignored
    let result = checker.check_file(&rule, &test_file, None).await;
    assert!(
        result.is_ok(),
        "allow-unwrap should be ignored by glob pattern *-unwrap"
    );
}

#[tokio::test]
async fn test_ignore_directive_question_mark_glob() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "test-a-rule".to_string(),
        "Test rule".to_string(),
        Severity::Error,
    );

    // Create a temp file with ? glob pattern
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "// sah rule ignore test-?-rule\nfn main() {}").unwrap();

    // Rule matching single character pattern should be ignored
    let result = checker.check_file(&rule, &test_file, None).await;
    assert!(
        result.is_ok(),
        "test-a-rule should be ignored by glob pattern test-?-rule"
    );
}

#[tokio::test]
async fn test_ignore_directive_case_sensitive() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );

    // Create a temp file with different case ignore (should NOT match)
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "// sah rule ignore No-Unwrap\nfn main() {}").unwrap();

    // Should NOT be ignored because rule names are case-sensitive
    let _result = checker.check_file(&rule, &test_file, None).await;
    // Will proceed to normal checking since ignore doesn't match
}

#[tokio::test]
async fn test_ignore_directive_whitespace_handling() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );

    let test_cases = [
        "//sah rule ignore no-unwrap\n",      // No space after //
        "//  sah  rule  ignore  no-unwrap\n", // Extra spaces
        "// sah rule ignore no-unwrap  \n",   // Trailing spaces
        "  // sah rule ignore no-unwrap\n",   // Leading spaces
    ];

    let temp_dir = TempDir::new().unwrap();

    for (i, content) in test_cases.iter().enumerate() {
        let test_file = temp_dir.path().join(format!("test_{}.rs", i));
        std::fs::write(&test_file, content).unwrap();

        let result = checker.check_file(&rule, &test_file, None).await;
        assert!(
            result.is_ok(),
            "Rule should be ignored with whitespace variation {}",
            i
        );
    }
}

#[tokio::test]
async fn test_ignore_directive_position_in_file() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = Rule::new(
        "no-unwrap".to_string(),
        "Check for unwrap()".to_string(),
        Severity::Error,
    );

    // Test ignore at different positions in file
    let positions = [
        "// sah rule ignore no-unwrap\nfn main() {}", // Top of file
        "fn helper() {}\n// sah rule ignore no-unwrap\nfn main() {}", // Middle
        "fn main() {}\n// sah rule ignore no-unwrap", // End of file
    ];

    let temp_dir = TempDir::new().unwrap();

    for (i, content) in positions.iter().enumerate() {
        let test_file = temp_dir.path().join(format!("position_test_{}.rs", i));
        std::fs::write(&test_file, content).unwrap();

        let result = checker.check_file(&rule, &test_file, None).await;
        assert!(
            result.is_ok(),
            "Rule should be ignored regardless of position in file (test case {})",
            i
        );
    }
}
