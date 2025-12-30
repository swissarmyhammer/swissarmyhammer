//! Integration tests for RuleChecker with partial template support
//!
//! These tests verify that rules can use {% include "_partials/..." %} to include
//! shared partial templates from the rule library.

use swissarmyhammer_common::error::SwissArmyHammerError;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::model::{LlamaAgentConfig, ModelConfig};
use swissarmyhammer_rules::{AgentConfig, Rule, RuleChecker, Severity};

/// Create a test agent config with local LlamaAgent for fast test execution
///
/// Uses a small test model instead of Claude Code to avoid API calls
/// and speed up test execution.
fn create_test_agent_config() -> AgentConfig {
    AgentConfig {
        model_config: ModelConfig::llama_agent(LlamaAgentConfig::for_testing()),
        mcp_config: None,
    }
}

/// Check if an error is due to agent unavailability and skip test if so
///
/// Returns true if the test should be skipped, false otherwise.
fn skip_if_agent_unavailable(err: &SwissArmyHammerError) -> bool {
    let err_string = err.to_string();
    if err_string.contains("agent")
        || err_string.contains("Agent")
        || err_string.contains("Claude CLI not found")
    {
        eprintln!("Skipping test - agent not available: {}", err);
        true
    } else {
        false
    }
}

#[tokio::test]
async fn test_rule_checker_with_partial_includes() {
    let agent_config = create_test_agent_config();
    let checker = match RuleChecker::new(agent_config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - failed to create checker: {}", e);
            return;
        }
    };

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
    let result = checker.check_file(&rule, &test_file).await;

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
    let agent_config = create_test_agent_config();
    let checker = match RuleChecker::new(agent_config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - failed to create checker: {}", e);
            return;
        }
    };

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
    let result = checker.check_file(&rule, &test_file).await;

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
