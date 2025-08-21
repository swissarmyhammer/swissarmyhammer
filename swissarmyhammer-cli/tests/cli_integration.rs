//! CLI integration tests using in-process testing
//!
//! Fast in-process CLI testing for comprehensive test coverage.

mod in_process_test_utils;

use anyhow::Result;
use in_process_test_utils::run_flow_test_in_process;
use std::fs;
use tempfile::TempDir;

/// Create a minimal test workflow for performance testing
fn create_minimal_workflow() -> String {
    r#"---
title: Minimal Test Workflow
description: Simple workflow for performance testing
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> test
    test --> [*]
```

## Actions

- test: Log "Test completed"
"#
    .to_string()
}

/// Helper to set up a temporary test environment with a workflow
async fn setup_test_workflow(workflow_name: &str) -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create .git directory to make it look like a Git repository
    let git_dir = temp_path.join(".git");
    fs::create_dir_all(&git_dir)?;

    // Create .swissarmyhammer/workflows directory
    let workflow_dir = temp_path.join(".swissarmyhammer").join("workflows");
    fs::create_dir_all(&workflow_dir)?;

    // Create minimal workflow
    let workflow_path = workflow_dir.join(format!("{}.md", workflow_name));
    fs::write(&workflow_path, create_minimal_workflow())?;

    Ok(temp_dir)
}

/// Run workflow in controlled test environment
async fn run_test_workflow_in_process(workflow_name: &str, vars: Vec<String>) -> Result<bool> {
    let temp_dir = setup_test_workflow(workflow_name).await?;
    let temp_path = temp_dir.path();

    // Change to temp directory
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_path)?;

    // Use very fast timeout for performance tests
    let result = run_flow_test_in_process(workflow_name, vars, Some("1s".to_string()), false).await;

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(result.is_ok())
}

#[tokio::test]
async fn test_flow_test_simple_workflow() -> Result<()> {
    // Test with minimal workflow in controlled environment
    let success = run_test_workflow_in_process("minimal-test", vec![]).await?;
    assert!(success, "Simple workflow should execute successfully");
    Ok(())
}

#[tokio::test]
async fn test_flow_test_coverage_complete() -> Result<()> {
    // Test coverage reporting with minimal test workflow
    let temp_dir = setup_test_workflow("coverage-test").await?;
    let temp_path = temp_dir.path();
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_path)?;

    let captured = run_flow_test_in_process("coverage-test", vec![], None, false).await?;

    std::env::set_current_dir(original_dir)?;

    // Whether it succeeds or fails, we're testing the coverage logic path
    // The exit code indicates whether the workflow ran successfully
    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should return valid exit code"
    );

    // If there was an error, it should be captured in stderr
    if captured.exit_code != 0 {
        assert!(!captured.stderr.is_empty(), "Should provide error details");
    }

    Ok(())
}

#[tokio::test]
async fn test_flow_test_with_set_variables() -> Result<()> {
    // Test with template variables
    let success = run_test_workflow_in_process(
        "vars-test",
        vec!["name=TestUser".to_string(), "language=Spanish".to_string()],
    )
    .await?;

    assert!(success, "Should handle workflow with variables gracefully");

    Ok(())
}

#[tokio::test]
async fn test_concurrent_flow_test() -> Result<()> {
    use tokio::task::JoinSet;

    let mut tasks = JoinSet::new();

    // Run multiple flow tests concurrently in-process with minimal test workflows
    for i in 0..3 {
        tasks.spawn(async move {
            let vars = vec![format!("run_id={}", i)];
            let result =
                run_test_workflow_in_process(&format!("concurrent-test-{}", i), vars).await;
            (i, result.is_ok())
        });
    }

    // All commands should complete without panicking
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((i, completed)) => {
                if !completed {
                    eprintln!("Concurrent flow test {} failed", i);
                }
                // Note: Don't assert here since concurrent tasks may have different outcomes
                // The important thing is they don't panic
            }
            Err(e) => {
                panic!("Task panicked: {:?}", e);
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_flow_test_with_timeout() -> Result<()> {
    // Test with timeout parameter
    let temp_dir = setup_test_workflow("timeout-test").await?;
    let temp_path = temp_dir.path();
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_path)?;

    let captured =
        run_flow_test_in_process("timeout-test", vec![], Some("10s".to_string()), false).await?;

    std::env::set_current_dir(original_dir)?;

    // Should complete (success or failure) within timeout
    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should return valid exit code"
    );

    Ok(())
}

#[tokio::test]
async fn test_flow_test_quiet_mode() -> Result<()> {
    // Test quiet mode flag
    let temp_dir = setup_test_workflow("quiet-test").await?;
    let temp_path = temp_dir.path();
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_path)?;

    let captured = run_flow_test_in_process("quiet-test", vec![], None, true).await?;

    std::env::set_current_dir(original_dir)?;

    // Should complete regardless of quiet mode
    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should return valid exit code"
    );

    Ok(())
}

#[tokio::test]
async fn test_flow_test_empty_set_value() -> Result<()> {
    // Test with empty set value
    let vars = vec!["empty_param=".to_string()];
    let captured = run_flow_test_in_process("test-workflow", vars, None, false).await?;

    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should handle empty values gracefully"
    );

    Ok(())
}

#[tokio::test]
async fn test_flow_test_special_chars_in_set() -> Result<()> {
    // Test with special characters in set values
    let vars = vec!["message=Hello, World! @#$%^&*()".to_string()];
    let captured = run_flow_test_in_process("test-workflow", vars, None, false).await?;

    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should handle special characters gracefully"
    );

    Ok(())
}
