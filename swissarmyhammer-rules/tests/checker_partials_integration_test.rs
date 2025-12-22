//! Integration tests for RuleChecker with partial template support
//!
//! These tests verify that rules can use {% include "_partials/..." %} to include
//! shared partial templates from the rule library.

use std::sync::Arc;
use swissarmyhammer_agent_executor::LlamaAgentExecutorWrapper;
use swissarmyhammer_common::error::SwissArmyHammerError;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::LlamaAgentConfig;
use swissarmyhammer_rules::{Rule, RuleChecker, Severity};

/// Create a test agent with default configuration
fn create_test_agent() -> Arc<LlamaAgentExecutorWrapper> {
    let config = LlamaAgentConfig::for_testing();
    let mcp_server = agent_client_protocol::McpServer::Http(
        agent_client_protocol::McpServerHttp::new("test", "http://localhost:8080/mcp"),
    );
    Arc::new(LlamaAgentExecutorWrapper::new(config, mcp_server))
}

/// Check if an error is due to agent unavailability and skip test if so
///
/// Returns true if the test should be skipped, false otherwise.
fn skip_if_agent_unavailable(err: &SwissArmyHammerError) -> bool {
    let err_string = err.to_string();
    if err_string.contains("agent") || err_string.contains("Agent") {
        eprintln!("Skipping test - agent not available: {}", err);
        true
    } else {
        false
    }
}

#[tokio::test]
async fn test_rule_checker_with_partial_includes() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    // Create a temp directory with partials and rules
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let partials_dir = temp_dir.join("_partials");
    std::fs::create_dir(&partials_dir).unwrap();

    // Create a partial template
    let partial_path = partials_dir.join("report-format.md");
    std::fs::write(
        &partial_path,
        "{% partial %}\n\nReport violations with line number and description.",
    )
    .unwrap();

    // Create a rule that uses the partial
    let rule_path = temp_dir.join("test-rule.md");
    std::fs::write(
        &rule_path,
        r#"---
title: Test Rule With Partial
severity: error
---

Check for issues in {{ language }} code.

{% include "_partials/report-format" %}

If no issues found, respond with "PASS".
"#,
    )
    .unwrap();

    // Load the rule using RuleLoader (simpler for testing)
    let rule_loader = swissarmyhammer_rules::RuleLoader::new();
    let rule = rule_loader
        .load_file(&rule_path)
        .expect("Rule should be loaded");

    // Create a test file to check
    let test_file = temp_dir.join("test.rs");
    std::fs::write(&test_file, "fn main() {}\n").unwrap();

    // Try to check the file with the rule that uses a partial
    let result = checker.check_file(&rule, &test_file, None).await;

    // If this fails, check if it's because partials aren't working
    if let Err(err) = result {
        // Skip if agent not available
        if skip_if_agent_unavailable(&err) {
            return;
        }

        let err_string = err.to_string();

        // This is the key test - we should NOT get "Partial does not exist" error
        assert!(
            !err_string.contains("Partial does not exist"),
            "Rule checker should support partials, but got error: {}",
            err_string
        );

        // Other errors are acceptable (violation, etc.)
    }
}

#[tokio::test]
async fn test_rule_with_builtin_partial() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    // Create a rule that uses a builtin partial (these ship with swissarmyhammer)
    let rule = Rule::new(
        "test-with-builtin".to_string(),
        r#"Check for issues.

{% include "_partials/report-format" %}
"#
        .to_string(),
        Severity::Error,
    );

    // Create a test file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("test.rs");
    std::fs::write(&test_file, "fn main() {}\n").unwrap();

    // Try to check with a rule that uses a builtin partial
    let result = checker.check_file(&rule, &test_file, None).await;

    // Should not fail with "Partial does not exist"
    if let Err(err) = result {
        if skip_if_agent_unavailable(&err) {
            return;
        }

        let err_string = err.to_string();

        assert!(
            !err_string.contains("Partial does not exist"),
            "Should support builtin partials, but got: {}",
            err_string
        );
    }
}
