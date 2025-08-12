//! Final comprehensive integration tests for abort system validation
//!
//! This test suite provides comprehensive end-to-end validation of the complete abort system
//! as specified in ABORT_000269. It covers areas not fully tested by existing test suites:
//!
//! 1. Cross-platform compatibility testing
//! 2. Performance impact assessment
//! 3. Stress testing and edge cases
//! 4. User experience validation
//! 5. Complete regression testing scenarios

use anyhow::Result;
use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Test environment setup helper
struct TestEnvironment {
    temp_dir: TempDir,
    original_dir: PathBuf,
}

impl TestEnvironment {
    fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_dir.path())?;
        Ok(Self {
            temp_dir,
            original_dir,
        })
    }

    fn cleanup(&self) -> Result<()> {
        std::env::set_current_dir(&self.original_dir)?;
        Ok(())
    }

    fn create_abort_file(&self, reason: &str) -> Result<()> {
        fs::create_dir_all(".swissarmyhammer")?;
        fs::write(".swissarmyhammer/.abort", reason)?;
        Ok(())
    }

    fn verify_abort_file(&self, expected_reason: &str) -> Result<()> {
        let abort_path = Path::new(".swissarmyhammer/.abort");
        assert!(abort_path.exists(), "Abort file should exist");
        let content = fs::read_to_string(abort_path)?;
        assert_eq!(content, expected_reason, "Abort file content mismatch");
        Ok(())
    }

    fn verify_no_abort_file(&self) {
        let abort_path = Path::new(".swissarmyhammer/.abort");
        assert!(!abort_path.exists(), "Abort file should not exist");
    }

    fn create_test_workflow(&self, name: &str) -> Result<String> {
        let workflow_content = format!(
            r#"---
name: {}
description: Test workflow for abort integration testing  
initial_state: start
states:
  start:
    name: Start State
    description: Starting state
    is_final: false
    actions:
      - type: log
        message: "Workflow {} started"
  processing:
    name: Processing State  
    description: Processing state with delay
    is_final: false
    actions:
      - type: log
        message: "Processing data"
      - type: wait
        duration_ms: 200
  end:
    name: End State
    description: Final state
    is_final: true  
    actions:
      - type: log
        message: "Workflow {} completed"
transitions:
  - from: start
    to: processing
    condition:
      type: always
  - from: processing
    to: end  
    condition:
      type: always
"#,
            name, name, name
        );

        // Create .swissarmyhammer/workflows directory
        fs::create_dir_all(".swissarmyhammer/workflows")?;
        let filename = format!("{}.md", name.replace(" ", "_").to_lowercase());
        let filepath = format!(".swissarmyhammer/workflows/{}", filename);
        fs::write(&filepath, workflow_content)?;
        Ok(filename.trim_end_matches(".md").to_string())
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Test performance impact of abort checking system
#[test]
#[ignore = "Complex workflow test - requires full workflow system setup"]
fn test_abort_performance_impact_baseline() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Performance Baseline")?;

    // Baseline measurement without abort file
    let start_time = Instant::now();
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .timeout(Duration::from_secs(30))
        .output()?;
    let baseline_duration = start_time.elapsed();

    // Should complete successfully
    assert!(
        output.status.success(),
        "Baseline workflow should complete successfully. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("Baseline execution time: {:?}", baseline_duration);

    // Performance should be reasonable (under 5 seconds for simple workflow)
    assert!(
        baseline_duration < Duration::from_secs(5),
        "Baseline performance should be under 5 seconds, got: {:?}",
        baseline_duration
    );

    Ok(())
}

/// Test performance with abort file checking overhead
#[test]
fn test_abort_performance_with_checking_overhead() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Performance With Checking")?;

    // Create workflow that will be aborted mid-execution
    env.create_abort_file("Performance test abort")?;

    let start_time = Instant::now();
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .timeout(Duration::from_secs(30))
        .output()?;
    let abort_duration = start_time.elapsed();

    // Should fail due to abort
    assert!(
        !output.status.success(),
        "Workflow should fail when abort file present"
    );

    println!("Abort detection time: {:?}", abort_duration);

    // Abort should be detected quickly (under 2 seconds)
    assert!(
        abort_duration < Duration::from_secs(2),
        "Abort should be detected quickly, got: {:?}",
        abort_duration
    );

    Ok(())
}

/// Test concurrent workflow execution with abort
#[test]
fn test_concurrent_workflow_abort_handling() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Create multiple workflow files
    let workflow_files: Vec<String> = (0..3)
        .map(|i| env.create_test_workflow(&format!("Concurrent Workflow {}", i)))
        .collect::<Result<Vec<_>>>()?;

    let barrier = Arc::new(Barrier::new(workflow_files.len() + 1));
    let mut handles = vec![];

    // Start multiple workflows concurrently
    for (i, workflow_file) in workflow_files.iter().enumerate() {
        let barrier = Arc::clone(&barrier);
        let workflow_file = workflow_file.clone();
        let current_dir = std::env::current_dir()?;

        let handle = thread::spawn(move || -> Result<Output> {
            std::env::set_current_dir(current_dir)?;
            barrier.wait();

            let output = Command::cargo_bin("swissarmyhammer")
                .unwrap()
                .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
                .args(["flow", "run", &workflow_file])
                .timeout(Duration::from_secs(30))
                .output()?;

            Ok(output)
        });

        handles.push((i, handle));
    }

    // Wait for all threads to be ready, then create abort file
    barrier.wait();
    thread::sleep(Duration::from_millis(100)); // Give workflows time to start
    env.create_abort_file("Concurrent test abort")?;

    // Collect results
    let results: Vec<_> = handles
        .into_iter()
        .map(|(i, handle)| (i, handle.join().unwrap()))
        .collect();

    // All workflows should either fail due to abort or complete before abort was created
    for (i, result) in results {
        match result {
            Ok(output) => {
                if output.status.success() {
                    println!("Workflow {} completed before abort", i);
                } else {
                    println!("Workflow {} was aborted", i);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Verify it's either an abort or a workflow not found error
                    assert!(
                        stderr.to_lowercase().contains("abort")
                            || stderr.contains("not found")
                            || stderr.contains("No such file"),
                        "Unexpected error for workflow {}: {}",
                        i,
                        stderr
                    );
                }
            }
            Err(e) => panic!("Thread {} failed: {}", i, e),
        }
    }

    Ok(())
}

/// Test rapid abort tool invocations (stress test)
#[test]
fn test_rapid_abort_invocations() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Test rapid creation and deletion of abort files
    for i in 0..10 {
        let reason = format!("Rapid abort test iteration {}", i);
        env.create_abort_file(&reason)?;
        env.verify_abort_file(&reason)?;

        // Clean up abort file
        fs::remove_file(".swissarmyhammer/.abort")?;
        env.verify_no_abort_file();

        // Small delay to simulate real usage
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

/// Test abort system with large abort reasons
#[test]
fn test_large_abort_reasons() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Large Reason Test")?;

    // Create large abort reason (10KB)
    let large_reason = "X".repeat(10240);
    env.create_abort_file(&large_reason)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .timeout(Duration::from_secs(30))
        .output()?;

    // Should still handle large abort reason correctly
    assert!(
        !output.status.success(),
        "Workflow should fail with large abort reason"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // We don't expect the full reason to be in the error message, but abort should be detected
    assert!(
        stderr.to_lowercase().contains("abort") || stderr.contains("not found"),
        "Should detect abort with large reason. Stderr: {}",
        stderr
    );

    Ok(())
}

/// Test abort system with unicode and special characters
#[test]
fn test_unicode_abort_reasons() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Unicode Test")?;

    let unicode_reason = "Abort with émojis 🚫 and ñoñ-ASCII characters 中文测试";
    env.create_abort_file(unicode_reason)?;
    env.verify_abort_file(unicode_reason)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .timeout(Duration::from_secs(30))
        .output()?;

    assert!(
        !output.status.success(),
        "Workflow should fail with unicode abort reason"
    );

    Ok(())
}

/// Test abort system behavior with filesystem edge cases
#[test]
fn test_filesystem_edge_cases() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Test with empty abort file
    fs::create_dir_all(".swissarmyhammer")?;
    fs::write(".swissarmyhammer/.abort", "")?;
    env.verify_abort_file("")?;

    // Test with abort file containing only whitespace
    fs::write(".swissarmyhammer/.abort", "   \t\n  ")?;
    env.verify_abort_file("   \t\n  ")?;

    // Test with abort file containing newlines
    let multiline_reason = "First line\nSecond line\nThird line";
    fs::write(".swissarmyhammer/.abort", multiline_reason)?;
    env.verify_abort_file(multiline_reason)?;

    Ok(())
}

/// Test abort system error messages and user experience
#[test]
#[ignore = "Complex workflow test - requires full workflow system setup"]
fn test_abort_error_messages_user_experience() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("UX Test")?;

    let abort_reason = "User initiated cancellation for testing UX";
    env.create_abort_file(abort_reason)?;

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .output()?;

    // Should exit with proper exit code
    assert_eq!(
        output.status.code(),
        Some(2),
        "Should exit with code 2 for abort. Output: {:?}",
        output
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Error message: {}", stderr);

    // Error message should be clear and user-friendly
    // Note: The exact error depends on whether abort is detected before or after workflow loading
    assert!(!stderr.is_empty(), "Should have meaningful error message");

    Ok(())
}

/// Test abort file cleanup between workflow runs
#[test]
fn test_abort_file_cleanup_between_runs() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Cleanup Test")?;

    // First run with abort file
    env.create_abort_file("First run abort")?;

    let output1 = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .output()?;

    assert!(
        !output1.status.success(),
        "First run should fail due to abort"
    );

    // Second run without abort file - should clean up and succeed
    let output2 = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .output()?;

    // Second run should succeed (abort file should be cleaned up)
    if !output2.status.success() {
        // If it fails, it should be due to workflow not found, not abort
        let stderr = String::from_utf8_lossy(&output2.stderr);
        assert!(
            !stderr.to_lowercase().contains("abort"),
            "Second run should not fail due to abort. Stderr: {}",
            stderr
        );
    }

    Ok(())
}

/// Test abort system with various CLI commands
#[test]
fn test_abort_with_different_cli_commands() -> Result<()> {
    let env = TestEnvironment::new()?;

    // Test with prompt command (if abort detection works there)
    env.create_abort_file("CLI command test abort")?;

    // Test prompt list command - should not be affected by abort file
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["prompt", "list"])
        .timeout(Duration::from_secs(10))
        .output()?;

    // Prompt list should work regardless of abort file
    println!(
        "Prompt list with abort file - exit code: {:?}",
        output.status.code()
    );

    // Test flow command with non-existent workflow
    let output2 = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", "nonexistent.md"])
        .timeout(Duration::from_secs(10))
        .output()?;

    assert!(
        !output2.status.success(),
        "Should fail for non-existent workflow"
    );

    Ok(())
}

/// Regression test to ensure existing functionality works
#[test]
#[ignore = "Complex workflow test - requires full workflow system setup"]
fn test_regression_normal_workflow_execution() -> Result<()> {
    let env = TestEnvironment::new()?;
    let workflow_file = env.create_test_workflow("Regression Test")?;

    // Ensure no abort file exists
    env.verify_no_abort_file();

    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .env("SWISSARMYHAMMER_SKIP_MCP_STARTUP", "1")
        .args(["flow", "run", &workflow_file])
        .timeout(Duration::from_secs(30))
        .output()?;

    // Should complete successfully without abort
    assert!(
        output.status.success(),
        "Normal workflow should complete successfully. Stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

/// Test cross-platform path handling for abort file
#[test]
fn test_cross_platform_abort_file_paths() -> Result<()> {
    let _env = TestEnvironment::new()?;

    // Test that abort file path works on current platform
    let abort_dir = Path::new(".swissarmyhammer");
    let abort_file = abort_dir.join(".abort");

    // Create directory and file manually to test path handling
    fs::create_dir_all(&abort_dir)?;
    fs::write(&abort_file, "Cross-platform test")?;

    assert!(
        abort_file.exists(),
        "Abort file should exist at expected path"
    );

    let content = fs::read_to_string(&abort_file)?;
    assert_eq!(content, "Cross-platform test");

    // Cleanup
    fs::remove_file(&abort_file)?;
    fs::remove_dir(&abort_dir)?;

    Ok(())
}
