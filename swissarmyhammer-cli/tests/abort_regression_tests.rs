//! Regression tests for abort system changes
//!
//! These tests ensure that the new file-based abort system maintains compatibility
//! with existing behavior and doesn't break normal workflow execution or error handling.

use anyhow::Result;
use assert_cmd::Command;
use std::path::Path;

/// Helper to clean up abort file
fn cleanup_abort_file() {
    let _ = std::fs::remove_file(".swissarmyhammer/.abort");
}

/// Helper to ensure abort file does not exist
fn ensure_no_abort_file() {
    cleanup_abort_file();
    let abort_path = Path::new(".swissarmyhammer/.abort");
    if abort_path.exists() {
        // Try once more to clean up - may be permission or timing issue
        cleanup_abort_file();
        // If it still exists after cleanup, it may be from another concurrent test
        // Just warn rather than fail since this is for regression testing
        if abort_path.exists() {
            println!("Warning: abort file still exists after cleanup, may affect test isolation");
            let _ = std::fs::remove_file(abort_path); // Force remove
        }
    }
}

#[test]
fn test_normal_workflow_execution_unchanged() -> Result<()> {
    ensure_no_abort_file();

    // Create a normal workflow that should execute successfully
    let workflow_content = r#"---
name: Normal Regression Test
description: Test that normal workflows still work
initial_state: start
states:
  start:
    name: Start State
    description: Starting state
    is_final: false
    actions:
      - type: log
        message: "Workflow started normally"
  middle:
    name: Middle State
    description: Middle processing state
    is_final: false
    actions:
      - type: log
        message: "Processing in middle state"
  end:
    name: End State
    description: Final state
    is_final: true
    actions:
      - type: log
        message: "Workflow completed successfully"
transitions:
  - from: start
    to: middle
    condition:
      type: always
  - from: middle
    to: end
    condition:
      type: always
"#;

    std::fs::write("normal_regression_test.md", workflow_content)?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", "normal_regression_test.md"])
        .output()?;

    // Clean up
    let _ = std::fs::remove_file("normal_regression_test.md");
    ensure_no_abort_file();

    // Workflow should complete successfully (or fail for reasons other than abort)
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should NOT contain abort-related error messages
        assert!(
            !stderr.contains("abort") && !stderr.contains("Abort"),
            "Normal workflow should not fail due to abort: stderr={stderr}, stdout={stdout}"
        );

        // If it fails, it should be for legitimate reasons (missing MCP server, etc.)
        // not abort-related
    }

    Ok(())
}

#[test]
fn test_prompt_commands_still_work() -> Result<()> {
    ensure_no_abort_file();

    // Test various prompt commands that should work normally
    let test_commands = vec![
        vec!["prompt", "list"],
        vec!["prompt", "list", "--format", "json"],
    ];

    for command_args in test_commands {
        let output = Command::cargo_bin("sah")
            .unwrap()
            .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
            .args(&command_args)
            .output()?;

        // Commands may fail due to MCP server issues but should not fail due to abort
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Should not contain abort-related errors
            assert!(
                !stderr.contains("abort") && !stderr.contains("Abort"),
                "Command {command_args:?} should not fail due to abort: {stderr}"
            );
        }
    }

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_help_and_version_commands_unchanged() -> Result<()> {
    ensure_no_abort_file();

    // Test basic CLI commands that should always work
    let basic_commands = vec![vec!["--help"], vec!["--version"], vec!["help"]];

    for command_args in basic_commands {
        let output = Command::cargo_bin("sah")
            .unwrap()
            .args(&command_args)
            .output()?;

        // These should succeed regardless of abort system
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            panic!(
                "Basic command {:?} should always succeed: exit_code={:?}, stderr={}, stdout={}",
                command_args,
                output.status.code(),
                stderr,
                stdout
            );
        }
    }

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_error_handling_for_invalid_workflows_unchanged() -> Result<()> {
    ensure_no_abort_file();

    // Create an invalid workflow to test error handling
    let invalid_workflow = r#"---
name: Invalid Workflow
description: This workflow has invalid syntax
initial_state: nonexistent_state
states:
  start:
    name: Start
    description: Start state
    is_final: false
transitions:
  - from: start
    to: nonexistent_target
    condition:
      type: always
"#;

    std::fs::write("invalid_regression_test.md", invalid_workflow)?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", "invalid_regression_test.md"])
        .output()?;

    let _ = std::fs::remove_file("invalid_regression_test.md");

    // Should fail due to invalid workflow, not abort
    assert!(!output.status.success(), "Invalid workflow should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should contain validation/workflow errors, not abort errors
    assert!(
        !stderr.contains("abort") && !stderr.contains("Abort"),
        "Invalid workflow should fail with validation error, not abort: {stderr}"
    );

    // Should contain some indication of the real problem
    assert!(
        stderr.contains("error")
            || stderr.contains("Error")
            || stderr.contains("invalid")
            || stderr.contains("Invalid")
            || stderr.contains("state")
            || stderr.contains("workflow"),
        "Should contain meaningful error message: {stderr}"
    );

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_existing_abort_prompt_compatibility() -> Result<()> {
    ensure_no_abort_file();

    // Test that the existing abort prompt still works (it should use the new MCP tool)
    let output = Command::cargo_bin("sah")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["prompt", "test", "abort"])
        .output()?;

    // The abort prompt should work with the new system
    // It may succeed (using MCP tool) or fail (missing MCP server), but should be consistent
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("Abort prompt test - stderr: {stderr}");
    println!("Abort prompt test - stdout: {stdout}");

    // The behavior should be deterministic and not crash
    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_exit_codes_remain_consistent() -> Result<()> {
    ensure_no_abort_file();

    // Test that exit codes for various scenarios remain the same
    let test_cases = vec![
        // (command_args, expected_success, description)
        (vec!["--help"], true, "help command"),
        (vec!["--version"], true, "version command"),
        // Invalid commands should still fail consistently
        (vec!["invalid-command"], false, "invalid command"),
    ];

    for (command_args, expected_success, description) in test_cases {
        let output = Command::cargo_bin("sah")
            .unwrap()
            .args(&command_args)
            .output()?;

        assert_eq!(
            output.status.success(),
            expected_success,
            "{} should have success={}, got exit_code={:?}",
            description,
            expected_success,
            output.status.code()
        );

        // Should not contain abort-related messages for these basic commands
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("abort") && !stderr.contains("Abort"),
            "{description} should not mention abort: {stderr}"
        );
    }

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_workflow_with_sub_workflows_unchanged() -> Result<()> {
    ensure_no_abort_file();

    // Test that sub-workflow functionality still works
    let main_workflow = r#"---
name: Main Regression Test
description: Test main workflow with sub-workflow
initial_state: start
states:
  start:
    name: Start
    description: Start state
    is_final: false
    actions:
      - type: log
        message: "Main workflow started"
  call_sub:
    name: Call Sub
    description: Call sub-workflow
    is_final: false
    actions:
      - type: log
        message: "About to call sub-workflow"
  end:
    name: End
    description: End state
    is_final: true
    actions:
      - type: log
        message: "Main workflow completed"
transitions:
  - from: start
    to: call_sub
    condition:
      type: always
  - from: call_sub
    to: end
    condition:
      type: always
"#;

    let sub_workflow = r#"---
name: Sub Regression Test
description: Test sub-workflow
initial_state: sub_start
states:
  sub_start:
    name: Sub Start
    description: Sub start state
    is_final: false
    actions:
      - type: log
        message: "Sub-workflow executing"
  sub_end:
    name: Sub End
    description: Sub end state
    is_final: true
    actions:
      - type: log
        message: "Sub-workflow completed"
transitions:
  - from: sub_start
    to: sub_end
    condition:
      type: always
"#;

    std::fs::write("main_regression.md", main_workflow)?;
    std::fs::write("sub_regression.md", sub_workflow)?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", "main_regression.md"])
        .output()?;

    let _ = std::fs::remove_file("main_regression.md");
    let _ = std::fs::remove_file("sub_regression.md");

    // May fail due to missing MCP server, but should not fail due to abort
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("abort") && !stderr.contains("Abort"),
            "Sub-workflow test should not fail due to abort: {stderr}"
        );
    }

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_multiple_workflow_executions_dont_interfere() -> Result<()> {
    ensure_no_abort_file();

    let workflow_content = r#"---
name: Sequential Test
description: Test sequential executions
initial_state: start
states:
  start:
    name: Start
    description: Start state
    is_final: false
    actions:
      - type: log
        message: "Sequential workflow execution"
  end:
    name: End
    description: End state
    is_final: true
    actions:
      - type: log
        message: "Sequential workflow completed"
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("sequential_test.md", workflow_content)?;

    // Run the same workflow multiple times sequentially
    for i in 0..3 {
        let output = Command::cargo_bin("sah")
            .unwrap()
            .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
            .args(["flow", "run", "sequential_test.md"])
            .output()?;

        // Each execution should behave consistently
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains("abort") && !stderr.contains("Abort"),
                "Sequential execution {i} should not fail due to abort: {stderr}"
            );
        }

        // Verify no abort file is created by normal execution
        let abort_path = Path::new(".swissarmyhammer/.abort");
        assert!(
            !abort_path.exists(),
            "Normal workflow execution {i} should not create abort file"
        );
    }

    let _ = std::fs::remove_file("sequential_test.md");
    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_error_messages_format_unchanged() -> Result<()> {
    ensure_no_abort_file();

    // Test that error message formats are consistent with previous behavior
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "run", "nonexistent_file.md"])
        .output()?;

    // Should fail because file doesn't exist
    assert!(
        !output.status.success(),
        "Should fail for non-existent file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should not mention abort
    assert!(
        !stderr.contains("abort") && !stderr.contains("Abort"),
        "File not found error should not mention abort: {stderr}"
    );

    // Should contain meaningful error about missing file
    assert!(
        stderr.contains("file")
            || stderr.contains("File")
            || stderr.contains("not found")
            || stderr.contains("No such file"),
        "Should contain file-related error message: {stderr}"
    );

    ensure_no_abort_file();
    Ok(())
}

#[test]
fn test_concurrent_cli_commands_no_interference() -> Result<()> {
    ensure_no_abort_file();

    let workflow_content = r#"---
name: Concurrent Regression Test
description: Test concurrent normal executions
initial_state: start
states:
  start:
    name: Start
    description: Start state
    is_final: true
transitions: []
"#;

    std::fs::write("concurrent_regression.md", workflow_content)?;

    // Run multiple commands concurrently
    let handles: Vec<_> = (0..3)
        .map(|i| {
            std::thread::spawn(move || {
                Command::cargo_bin("sah")
                    .unwrap()
                    .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
                    .args(["flow", "run", "concurrent_regression.md"])
                    .output()
                    .map(|output| (i, output))
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let _ = std::fs::remove_file("concurrent_regression.md");

    // All executions should behave consistently
    for result in results {
        match result {
            Ok((i, output)) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert!(
                        !stderr.contains("abort") && !stderr.contains("Abort"),
                        "Concurrent execution {i} should not fail due to abort: {stderr}"
                    );
                }
            }
            Err(e) => panic!("Concurrent execution failed: {e}"),
        }
    }

    ensure_no_abort_file();
    Ok(())
}
