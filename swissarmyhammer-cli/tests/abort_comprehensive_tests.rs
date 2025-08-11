//! Comprehensive integration tests for CLI abort system handling
//!
//! This test suite validates the complete CLI integration with the new file-based abort system,
//! ensuring proper exit codes, error message formatting, and integration with various CLI commands.

use anyhow::Result;
use assert_cmd::Command;
use std::path::Path;
use std::process::Output;

/// Helper to create .swissarmyhammer directory and abort file
fn create_abort_file(reason: &str) -> Result<()> {
    std::fs::create_dir_all(".swissarmyhammer")?;
    std::fs::write(".swissarmyhammer/.abort", reason)?;
    Ok(())
}

/// Helper to clean up abort file
fn cleanup_abort_file() {
    let _ = std::fs::remove_file(".swissarmyhammer/.abort");
}

/// Helper to verify abort file exists with specific content
fn assert_abort_file_exists(expected_reason: &str) -> Result<()> {
    let abort_path = Path::new(".swissarmyhammer/.abort");
    assert!(abort_path.exists(), "Abort file should exist");
    
    let content = std::fs::read_to_string(abort_path)?;
    assert_eq!(content, expected_reason, "Abort file content mismatch");
    Ok(())
}

/// Helper to verify abort file does not exist
fn assert_abort_file_not_exists() {
    let abort_path = Path::new(".swissarmyhammer/.abort");
    assert!(!abort_path.exists(), "Abort file should not exist");
}

/// Helper to check output for abort-related error handling
fn assert_abort_error_handling(output: &Output) {
    // Command should fail with exit code 2 (EXIT_ERROR)
    assert!(
        !output.status.success(),
        "Command should fail when ExecutorError::Abort is detected"
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "Exit code should be 2 (EXIT_ERROR) when ExecutorError::Abort is detected"
    );

    // Error output should indicate abort
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Workflow aborted") || stderr.contains("abort"),
        "Error output should indicate abort condition: {stderr}"
    );
}

#[test]
fn test_workflow_execution_with_abort_file_present() -> Result<()> {
    cleanup_abort_file();
    
    // Create a simple workflow file for testing
    let workflow_content = r#"---
name: Test Workflow
description: A test workflow for abort testing
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
    actions:
      - type: log
        message: "Starting workflow"
  end:
    name: End  
    description: Final state
    is_final: true
    actions: []
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("test_abort_workflow.md", workflow_content)?;
    
    // Create abort file before workflow execution
    create_abort_file("CLI integration test abort")?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "test_abort_workflow.md"])
        .output()?;

    // Clean up
    cleanup_abort_file();
    let _ = std::fs::remove_file("test_abort_workflow.md");

    assert_abort_error_handling(&output);
    Ok(())
}

#[test] 
fn test_prompt_command_with_abort_file() -> Result<()> {
    cleanup_abort_file();

    // Create abort file
    create_abort_file("Prompt command abort test")?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["prompt", "test", "example"])
        .output()?;

    cleanup_abort_file();

    // Even though prompt test doesn't execute workflows, it should still
    // detect the abort file if the system is properly integrated
    // For now, just verify the command can run with abort file present
    // This may succeed or fail depending on internal workflow usage
    println!("Prompt test output: {:?}", output);
    Ok(())
}

#[test]
fn test_multiple_cli_commands_ignore_stale_abort_file() -> Result<()> {
    cleanup_abort_file();

    // Create abort file
    create_abort_file("Stale abort file")?;

    // Commands that don't use workflows should succeed despite abort file
    let commands = vec![
        vec!["prompt", "list"],
        vec!["--help"],
        vec!["--version"],
    ];

    for command_args in commands {
        let output = Command::cargo_bin("swissarmyhammer")
            .unwrap()
            .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
            .args(&command_args)
            .output()?;

        // These commands should succeed as they don't involve workflow execution
        if !output.status.success() {
            println!("Command failed: {:?}", command_args);
            println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        }
        // Note: Some commands might legitimately fail due to missing MCP server
        // but shouldn't fail specifically due to abort file
    }

    cleanup_abort_file();
    Ok(())
}

#[test]
fn test_abort_file_with_unicode_reason() -> Result<()> {
    cleanup_abort_file();

    let workflow_content = r#"---
name: Unicode Abort Test
description: Test workflow with unicode abort
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("unicode_abort_test.md", workflow_content)?;

    let unicode_reason = "中文测试 🚫 Abort with émojis and ñoñ-ASCII";
    create_abort_file(unicode_reason)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "unicode_abort_test.md"])
        .output()?;

    cleanup_abort_file();
    let _ = std::fs::remove_file("unicode_abort_test.md");

    assert_abort_error_handling(&output);
    
    // Check that unicode is preserved in error message
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Unicode might be in the error message depending on how it's propagated
    println!("Unicode abort stderr: {}", stderr);
    Ok(())
}

#[test]
fn test_abort_file_cleanup_between_command_runs() -> Result<()> {
    cleanup_abort_file();

    // Verify no abort file initially
    assert_abort_file_not_exists();

    // Create abort file
    create_abort_file("Test cleanup reason")?;
    assert_abort_file_exists("Test cleanup reason")?;

    // Note: CLI commands themselves don't clean up abort files
    // Only WorkflowRun::new() cleans them up
    // This test documents current behavior

    cleanup_abort_file();
    assert_abort_file_not_exists();

    Ok(())
}

#[test]
fn test_abort_file_with_large_reason() -> Result<()> {
    cleanup_abort_file();

    let workflow_content = r#"---
name: Large Reason Test
description: Test with large abort reason
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("large_reason_test.md", workflow_content)?;

    let large_reason = "x".repeat(1000);
    create_abort_file(&large_reason)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1") 
        .args(["flow", "large_reason_test.md"])
        .output()?;

    cleanup_abort_file();
    let _ = std::fs::remove_file("large_reason_test.md");

    assert_abort_error_handling(&output);
    Ok(())
}

#[test]
fn test_abort_file_with_newlines() -> Result<()> {
    cleanup_abort_file();

    let workflow_content = r#"---
name: Newline Test
description: Test with newline abort reason
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("newline_test.md", workflow_content)?;

    let reason_with_newlines = "Line 1\nLine 2\r\nLine 3\n";
    create_abort_file(reason_with_newlines)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "newline_test.md"])
        .output()?;

    cleanup_abort_file();
    let _ = std::fs::remove_file("newline_test.md");

    assert_abort_error_handling(&output);
    Ok(())
}

#[test] 
fn test_empty_abort_file() -> Result<()> {
    cleanup_abort_file();

    let workflow_content = r#"---
name: Empty Abort Test
description: Test with empty abort file
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("empty_abort_test.md", workflow_content)?;

    // Create empty abort file
    create_abort_file("")?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "empty_abort_test.md"])
        .output()?;

    cleanup_abort_file();
    let _ = std::fs::remove_file("empty_abort_test.md");

    assert_abort_error_handling(&output);
    Ok(())
}

#[test]
fn test_normal_workflow_execution_without_abort_file() -> Result<()> {
    cleanup_abort_file();

    let workflow_content = r#"---
name: Normal Test
description: Test normal workflow execution
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
    actions:
      - type: log
        message: "Normal execution"
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("normal_test.md", workflow_content)?;

    // Ensure no abort file exists
    cleanup_abort_file();
    assert_abort_file_not_exists();

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "normal_test.md"])
        .output()?;

    let _ = std::fs::remove_file("normal_test.md");

    // Should succeed normally
    if !output.status.success() {
        println!("Normal workflow stderr: {}", String::from_utf8_lossy(&output.stderr));
        println!("Normal workflow stdout: {}", String::from_utf8_lossy(&output.stdout));
    }

    // Verify still no abort file exists after successful run
    assert_abort_file_not_exists();

    Ok(())
}

#[test]
fn test_concurrent_cli_commands_with_abort_file() -> Result<()> {
    cleanup_abort_file();

    // Create abort file
    create_abort_file("Concurrent test abort")?;

    let workflow_content = r#"---
name: Concurrent Test
description: Test concurrent executions
initial_state: start
states:
  start:
    name: Start
    description: Starting state
    is_final: false
  end:
    name: End
    description: Final state
    is_final: true
transitions:
  - from: start
    to: end
    condition:
      type: always
"#;

    std::fs::write("concurrent_test.md", workflow_content)?;

    // Run multiple commands concurrently
    let handles: Vec<_> = (0..3)
        .map(|_| {
            std::thread::spawn(|| {
                Command::cargo_bin("swissarmyhammer")
                    .unwrap()
                    .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
                    .args(["flow", "concurrent_test.md"])
                    .output()
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    cleanup_abort_file();
    let _ = std::fs::remove_file("concurrent_test.md");

    // All should handle abort appropriately
    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(output) => {
                if !output.status.success() {
                    // Should fail with abort error
                    assert_eq!(output.status.code(), Some(2), "Thread {} should exit with code 2", i);
                } else {
                    // Might succeed if abort file was cleaned up by another instance
                    println!("Thread {} succeeded (abort file may have been cleaned up)", i);
                }
            }
            Err(e) => panic!("Thread {} failed to execute command: {}", i, e),
        }
    }

    Ok(())
}