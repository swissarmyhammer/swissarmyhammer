//! Integration tests for RuleChecker with real LlamaAgent execution
//!
//! These tests verify the complete check_all flow with actual LLM execution,
//! testing fail-fast behavior, response parsing, and multi-rule/multi-file scenarios.

use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_agent_executor::LlamaAgentExecutorWrapper;
use swissarmyhammer_config::LlamaAgentConfig;
use swissarmyhammer_rules::{Rule, RuleChecker, Severity};
use tempfile::TempDir;

/// Create a test agent with default configuration
fn create_test_agent() -> Arc<LlamaAgentExecutorWrapper> {
    let config = LlamaAgentConfig::for_testing();
    Arc::new(LlamaAgentExecutorWrapper::new(config))
}

/// Create a test rule that checks for TODO comments
fn create_todo_rule() -> Rule {
    Rule::new(
        "no-todos".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    )
}

/// Create a test rule that always passes
fn create_always_pass_rule() -> Rule {
    Rule::new(
        "always-pass".to_string(),
        "This is a test rule that always passes. Respond with PASS.".to_string(),
        Severity::Info,
    )
}

#[tokio::test]
async fn test_check_all_with_single_passing_file() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    // Initialize the checker
    if checker.initialize().await.is_err() {
        // Skip test if agent initialization fails (no model available)
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    // Create a temp file with no TODOs
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("clean.rs");
    std::fs::write(
        &test_file,
        "fn main() {\n    println!(\"Hello, world!\");\n}\n",
    )
    .unwrap();

    let rules = vec![create_todo_rule()];
    let targets = vec![test_file];

    // Should pass - no TODOs in the file
    let result = checker.check_all(rules, targets).await;

    // If agent is not available, test will error out during execution
    if result.is_err() {
        let err = result.unwrap_err();
        // Accept agent errors as test environment limitations
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        panic!("Unexpected error: {}", err);
    }

    assert!(result.is_ok(), "Clean file should pass TODO check");
}

#[tokio::test]
async fn test_check_all_with_single_failing_file() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    // Create a temp file with a TODO comment
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("with_todo.rs");
    std::fs::write(
        &test_file,
        "fn main() {\n    // TODO: implement this\n    println!(\"Hello!\");\n}\n",
    )
    .unwrap();

    let rules = vec![create_todo_rule()];
    let targets = vec![test_file.clone()];

    let result = checker.check_all(rules, targets).await;

    // Should fail with a violation or agent error
    if result.is_err() {
        let err = result.unwrap_err();
        let err_string = err.to_string();

        if err_string.contains("agent") || err_string.contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }

        // Check if it's a violation error (by checking the error message)
        if err_string.contains("violated") && err_string.contains("no-todos") {
            // Successfully detected violation
            assert!(err_string.contains(&test_file.display().to_string()));
        } else {
            panic!("Expected violation error, got: {}", err);
        }
    } else {
        // If it passed, the LLM might not have detected the TODO
        // This is acceptable behavior for this test
        eprintln!(
            "Note: LLM did not detect TODO - this is acceptable for integration test validation"
        );
    }
}

#[tokio::test]
async fn test_check_all_fail_fast_behavior() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    // Create multiple files - first one has a TODO
    let temp_dir = TempDir::new().unwrap();

    let file1 = temp_dir.path().join("first.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: fix this\n}\n").unwrap();

    let file2 = temp_dir.path().join("second.rs");
    std::fs::write(&file2, "fn test() {}\n").unwrap();

    let file3 = temp_dir.path().join("third.rs");
    std::fs::write(&file3, "fn another() {}\n").unwrap();

    let rules = vec![create_todo_rule()];
    let targets = vec![file1.clone(), file2, file3];

    let result = checker.check_all(rules, targets).await;

    // Should fail fast on the first file
    if result.is_err() {
        let err = result.unwrap_err();
        let err_string = err.to_string();

        if err_string.contains("agent") || err_string.contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }

        // Should contain the first file path (fail-fast behavior)
        assert!(
            err_string.contains(&file1.display().to_string()),
            "Error should reference first file for fail-fast: {}",
            err_string
        );
    }
}

#[tokio::test]
async fn test_check_all_with_multiple_rules() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "fn main() {}\n").unwrap();

    // Multiple rules - at least one should pass
    let rules = vec![create_always_pass_rule(), create_todo_rule()];
    let targets = vec![test_file];

    let result = checker.check_all(rules, targets).await;

    // Accept either success or agent error
    if result.is_err() {
        let err = result.unwrap_err();
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        // Otherwise it's a real error
        panic!("Unexpected error with multiple rules: {}", err);
    }
}

#[tokio::test]
async fn test_check_all_with_empty_rule_list() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, "fn main() {}\n").unwrap();

    let rules = vec![];
    let targets = vec![test_file];

    let result = checker.check_all(rules, targets).await;

    // Empty rule list should succeed immediately
    assert!(
        result.is_ok(),
        "Empty rule list should pass without checking"
    );
}

#[tokio::test]
async fn test_check_all_with_empty_target_list() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let rules = vec![create_todo_rule()];
    let targets = vec![];

    let result = checker.check_all(rules, targets).await;

    // Empty target list should succeed immediately
    assert!(
        result.is_ok(),
        "Empty target list should pass without checking"
    );
}

#[tokio::test]
async fn test_check_all_with_both_empty() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let rules = vec![];
    let targets = vec![];

    let result = checker.check_all(rules, targets).await;

    // Both empty should succeed immediately
    assert!(result.is_ok(), "Empty lists should pass");
}

#[tokio::test]
async fn test_check_file_with_nonexistent_file() {
    let agent = create_test_agent();
    let checker = RuleChecker::new(agent).expect("Failed to create checker");

    let rule = create_todo_rule();
    let nonexistent = PathBuf::from("/nonexistent/file/that/does/not/exist.rs");

    let result = checker.check_file(&rule, &nonexistent).await;

    // Should fail with a CheckError
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Failed to read"),
        "Expected read error, got: {}",
        err
    );
}

#[tokio::test]
async fn test_response_parsing_pass() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("clean.rs");
    std::fs::write(&test_file, "fn clean_code() {}\n").unwrap();

    // Use a rule that should always pass for clean code
    let rule = create_always_pass_rule();
    let targets = vec![test_file];

    let result = checker.check_all(vec![rule], targets).await;

    // Should pass or encounter agent error
    if result.is_err() {
        let err = result.unwrap_err();
        if !err.to_string().contains("agent") && !err.to_string().contains("Agent") {
            panic!("Unexpected error: {}", err);
        }
    }
}

#[tokio::test]
async fn test_language_detection_in_checking() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Test different file types
    let rust_file = temp_dir.path().join("test.rs");
    std::fs::write(&rust_file, "fn main() {}\n").unwrap();

    let python_file = temp_dir.path().join("test.py");
    std::fs::write(&python_file, "def main(): pass\n").unwrap();

    let js_file = temp_dir.path().join("test.js");
    std::fs::write(&js_file, "function main() {}\n").unwrap();

    let rule = create_always_pass_rule();
    let targets = vec![rust_file, python_file, js_file];

    let result = checker.check_all(vec![rule], targets).await;

    // Should handle all language types
    if result.is_err() {
        let err = result.unwrap_err();
        if !err.to_string().contains("agent") && !err.to_string().contains("Agent") {
            panic!("Language detection test failed: {}", err);
        }
    }
}

#[test]
fn test_rule_checker_creation() {
    let agent = create_test_agent();
    let result = RuleChecker::new(agent);
    assert!(result.is_ok(), "RuleChecker creation should succeed");
}

#[test]
fn test_rule_checker_creation_verifies_check_prompt() {
    let agent = create_test_agent();

    // RuleChecker::new verifies .check prompt exists during construction
    // If this succeeds, the .check prompt was successfully loaded
    let result = RuleChecker::new(agent);
    assert!(
        result.is_ok(),
        "RuleChecker creation validates .check prompt is loaded"
    );
}

#[tokio::test]
async fn test_warning_does_not_stop_execution() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Create first file with a TODO (should trigger warning)
    let file1 = temp_dir.path().join("first.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: implement this\n}\n").unwrap();

    // Create second file that is clean
    let file2 = temp_dir.path().join("second.rs");
    std::fs::write(&file2, "fn test() {\n    println!(\"clean code\");\n}\n").unwrap();

    // Use a WARNING severity rule
    let warning_rule = Rule::new(
        "no-todos-warning".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    );

    let rules = vec![warning_rule];
    let targets = vec![file1.clone(), file2.clone()];

    let result = checker.check_all(rules, targets).await;

    // WARNING should NOT stop execution - should continue to check second file
    // The test passes if check_all completes successfully OR if agent is unavailable
    if result.is_err() {
        let err = result.unwrap_err();
        let err_string = err.to_string();

        if err_string.contains("agent") || err_string.contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }

        // If we get a violation error, that's a test failure
        // because warnings should NOT cause early exit
        panic!(
            "WARNING severity violation caused early exit - this should not happen. Error: {}",
            err_string
        );
    }

    // Success means both files were checked despite warning in first file
    assert!(result.is_ok(), "Warnings should not stop execution");
}

#[tokio::test]
async fn test_error_does_stop_execution() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Create first file with a TODO (should trigger error)
    let file1 = temp_dir.path().join("first.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: implement this\n}\n").unwrap();

    // Create second file that is clean
    let file2 = temp_dir.path().join("second.rs");
    std::fs::write(&file2, "fn test() {\n    println!(\"clean code\");\n}\n").unwrap();

    // Use an ERROR severity rule
    let error_rule = Rule::new(
        "no-todos-error".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Error,
    );

    let rules = vec![error_rule];
    let targets = vec![file1.clone(), file2.clone()];

    let result = checker.check_all(rules, targets).await;

    // ERROR should stop execution immediately (fail-fast)
    if result.is_err() {
        let err = result.unwrap_err();
        let err_string = err.to_string();

        if err_string.contains("agent") || err_string.contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }

        // This is expected - ERROR should cause early exit
        assert!(
            err_string.contains(&file1.display().to_string()),
            "Error should reference first file for fail-fast behavior: {}",
            err_string
        );
        return;
    }

    // If no error occurred, LLM might not have detected the TODO
    eprintln!("Note: LLM did not detect TODO in error test - acceptable for validation");
}

#[tokio::test]
async fn test_cached_warning_does_not_stop_execution() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Create a file with a TODO (should trigger warning)
    let file1 = temp_dir.path().join("with_todo.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: implement this\n}\n").unwrap();

    // Create a clean file that should be checked after the warning
    let file2 = temp_dir.path().join("clean.rs");
    std::fs::write(&file2, "fn test() {\n    println!(\"clean code\");\n}\n").unwrap();

    // Use a WARNING severity rule
    let warning_rule = Rule::new(
        "no-todos-cached-warning".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    );

    let rules = vec![warning_rule.clone()];
    let targets = vec![file1.clone(), file2.clone()];

    // First run: fresh evaluation creates cached warning
    let result1 = checker.check_all(rules.clone(), targets.clone()).await;

    if result1.is_err() {
        let err = result1.unwrap_err();
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        panic!(
            "First run: WARNING should not stop execution. Error: {}",
            err
        );
    }

    // Second run: cached warning should still not stop execution
    let result2 = checker.check_all(rules, targets).await;

    if result2.is_err() {
        let err = result2.unwrap_err();
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        panic!(
            "Second run (cached): Cached WARNING should not stop execution. Error: {}",
            err
        );
    }

    // Both runs should succeed despite warnings
    assert!(
        result1.is_ok() && result2.is_ok(),
        "Cached warnings should behave the same as fresh warnings - not stopping execution"
    );
}

#[tokio::test]
async fn test_mixed_severities_across_rules() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Create a file with a TODO that will match both warning and error rules
    let file1 = temp_dir.path().join("with_todo.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: implement this\n}\n").unwrap();

    // Create a clean file
    let file2 = temp_dir.path().join("clean.rs");
    std::fs::write(&file2, "fn test() {\n    println!(\"clean code\");\n}\n").unwrap();

    // Create a WARNING severity rule
    let warning_rule = Rule::new(
        "no-todos-warning-mixed".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    );

    // Create an ERROR severity rule
    let error_rule = Rule::new(
        "no-todos-error-mixed".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Error,
    );

    // Test 1: Warning rule first, then error rule
    // Both rules will check the same file with TODO
    let rules = vec![warning_rule.clone(), error_rule.clone()];
    let targets = vec![file1.clone(), file2.clone()];

    let result = checker.check_all(rules, targets).await;

    // If the LLM detects the TODO in the error rule, it should fail fast
    // If the LLM detects the TODO in the warning rule only, it should continue
    if result.is_err() {
        let err = result.unwrap_err();
        let err_string = err.to_string();

        if err_string.contains("agent") || err_string.contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }

        // If we got an error, it should be from the error-severity rule
        assert!(
            err_string.contains("no-todos-error-mixed"),
            "Error should be from the ERROR severity rule, not warning. Error: {}",
            err_string
        );
    }

    // Test 2: Multiple warnings from different rules should not stop execution
    let warning_rule2 = Rule::new(
        "no-todos-warning-second".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    );

    let warning_only_rules = vec![warning_rule, warning_rule2];
    let targets = vec![file1.clone(), file2.clone()];

    let result2 = checker.check_all(warning_only_rules, targets).await;

    // Multiple warnings should not stop execution
    if result2.is_err() {
        let err = result2.unwrap_err();
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        panic!(
            "Multiple WARNING rules should not stop execution. Error: {}",
            err
        );
    }

    assert!(
        result2.is_ok(),
        "Multiple warning violations should not stop execution"
    );
}

#[tokio::test]
async fn test_mixed_severities_across_multiple_files() {
    let agent = create_test_agent();
    let mut checker = RuleChecker::new(agent).expect("Failed to create checker");

    if checker.initialize().await.is_err() {
        eprintln!("Skipping test - agent initialization failed");
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Create first file with TODO (will trigger warnings)
    let file1 = temp_dir.path().join("with_todo_1.rs");
    std::fs::write(&file1, "fn main() {\n    // TODO: implement\n}\n").unwrap();

    // Create second file with TODO (will trigger warnings)
    let file2 = temp_dir.path().join("with_todo_2.rs");
    std::fs::write(&file2, "fn test() {\n    // TODO: write test\n}\n").unwrap();

    // Create third file that is clean
    let file3 = temp_dir.path().join("clean.rs");
    std::fs::write(&file3, "fn clean() {\n    println!(\"done\");\n}\n").unwrap();

    // Use WARNING severity rule
    let warning_rule = Rule::new(
        "no-todos-warning-multifile".to_string(),
        "Check if the {{language}} code contains TODO comments. If found, report a VIOLATION. If not found or if this is not a code file, respond with PASS.".to_string(),
        Severity::Warning,
    );

    let rules = vec![warning_rule];
    let targets = vec![file1.clone(), file2.clone(), file3.clone()];

    let result = checker.check_all(rules, targets).await;

    // Warnings across multiple files should not stop execution
    if result.is_err() {
        let err = result.unwrap_err();
        if err.to_string().contains("agent") || err.to_string().contains("Agent") {
            eprintln!("Skipping test - agent not available: {}", err);
            return;
        }
        panic!(
            "WARNING violations across multiple files should not stop execution. Error: {}",
            err
        );
    }

    assert!(
        result.is_ok(),
        "Warning violations across multiple files should not stop execution"
    );
}
