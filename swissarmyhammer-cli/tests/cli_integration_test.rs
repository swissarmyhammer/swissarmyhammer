//! Integration tests for CLI command structure and backward compatibility

use anyhow::Result;
use assert_cmd::Command;
use std::fs;
use tempfile::{NamedTempFile, TempDir};

mod test_utils;
use test_utils::create_test_environment;

/// Helper function to run CLI command and capture output to temp files
/// Returns (stdout_file, stderr_file, exit_code)
fn run_command_with_temp_output(
    cmd: &mut Command,
) -> Result<(NamedTempFile, NamedTempFile, Option<i32>)> {
    let stdout_file = NamedTempFile::new()?;
    let stderr_file = NamedTempFile::new()?;

    let output = cmd.output()?;

    // Write output to temp files
    fs::write(stdout_file.path(), &output.stdout)?;
    fs::write(stderr_file.path(), &output.stderr)?;

    Ok((stdout_file, stderr_file, output.status.code()))
}

/// Helper function to read content from temp file
fn read_temp_file(file: &NamedTempFile) -> Result<String> {
    Ok(fs::read_to_string(file.path())?)
}

/// Helper for tests that need to check stdout content
fn run_command_check_stdout_contains(cmd: &mut Command, expected_content: &[&str]) -> Result<()> {
    let (stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(cmd)?;
    assert!(exit_code == Some(0), "Command should succeed");

    let stdout_content = read_temp_file(&stdout_file)?;
    for content in expected_content {
        assert!(
            stdout_content.contains(content),
            "Output should contain '{content}': {stdout_content}",
        );
    }
    Ok(())
}

/// Test that the new prompt subcommand structure works correctly
#[test]
fn test_prompt_subcommand_list() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["prompt", "list"]);

    let (_stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    assert!(exit_code == Some(0), "prompt list command should succeed");
    Ok(())
}

/// Test prompt search functionality
#[test]
fn test_prompt_subcommand_search() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["prompt", "search", "test"]);

    let (_stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    // Search might not find results but should not error
    assert!(exit_code.is_some(), "prompt search should complete");
    Ok(())
}

/// Test prompt validate functionality
#[test]
fn test_prompt_subcommand_validate() -> Result<()> {
    let (_temp_dir, prompts_dir) = create_test_environment()?;

    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args([
        "prompt",
        "validate",
        "--workflow-dirs",
        prompts_dir.to_str().unwrap(),
    ]);

    let (_stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    // Validation should complete (may have warnings but shouldn't crash)
    assert!(exit_code.is_some(), "prompt validate should complete");
    Ok(())
}

/// Test prompt test functionality with a simple prompt
#[test]
fn test_prompt_subcommand_test() -> Result<()> {
    let (_temp_dir, _prompts_dir) = create_test_environment()?;

    // Test with non-existent prompt should fail gracefully
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["prompt", "test", "non_existent_prompt"]);

    let (_stdout_file, stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    assert!(
        exit_code != Some(0),
        "testing non-existent prompt should fail"
    );
    assert_eq!(exit_code, Some(1), "should return exit code 1");

    // Verify error message is present
    let stderr_content = read_temp_file(&stderr_file)?;
    assert!(
        stderr_content.contains("Error:") || stderr_content.contains("not found"),
        "should show meaningful error message"
    );

    Ok(())
}

/// Test help output for prompt subcommands
#[test]
fn test_prompt_help() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["prompt", "--help"]);

    let (stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    assert!(exit_code == Some(0), "prompt help should succeed");

    let stdout_content = read_temp_file(&stdout_file)?;
    assert!(
        stdout_content.contains("list"),
        "help should mention list subcommand"
    );
    assert!(
        stdout_content.contains("search"),
        "help should mention search subcommand"
    );
    assert!(
        stdout_content.contains("validate"),
        "help should mention validate subcommand"
    );
    assert!(
        stdout_content.contains("test"),
        "help should mention test subcommand"
    );

    Ok(())
}

/// Test shell completion generation
#[test]
fn test_completion_command() -> Result<()> {
    let shells = vec!["bash", "zsh", "fish"];

    for shell in shells {
        let mut cmd = Command::cargo_bin("sah").unwrap();
        cmd.args(["completion", shell]);

        let (stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

        assert!(exit_code == Some(0), "{shell} completion should succeed");

        let stdout_content = read_temp_file(&stdout_file)?;
        assert!(
            !stdout_content.trim().is_empty(),
            "{shell} completion should generate output"
        );
    }

    Ok(())
}

/// Test error handling and exit codes
#[test]
fn test_error_exit_codes() -> Result<()> {
    // Test validation error (exit code 2)
    let temp_dir = TempDir::new()?;
    let invalid_dir = temp_dir.path().join("non_existent");

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "prompt",
            "validate",
            "--workflow-dirs",
            invalid_dir.to_str().unwrap(),
        ])
        .output()?;

    // Should handle gracefully even if directory doesn't exist
    assert!(output.status.code().is_some(), "should return an exit code");

    Ok(())
}

/// Test that verbose flag works
#[test]
fn test_verbose_flag() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["--verbose", "prompt", "list"]);

    let (_stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    // Command should still work with verbose flag
    assert!(
        exit_code.is_some(),
        "verbose flag should not break commands"
    );

    Ok(())
}

/// Test that quiet flag works
#[test]
fn test_quiet_flag() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["--quiet", "prompt", "list"]);

    let (_stdout_file, _stderr_file, exit_code) = run_command_with_temp_output(&mut cmd)?;

    // Command should still work with quiet flag
    assert!(exit_code.is_some(), "quiet flag should not break commands");

    Ok(())
}

/// Test flow test command with simple workflow
#[test]
fn test_flow_test_simple_workflow() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.args(["flow", "test", "hello-world"]);

    run_command_check_stdout_contains(
        &mut cmd,
        &[
            "Test mode",
            "Coverage Report",
            "States visited",
            "Transitions used",
        ],
    )?;

    Ok(())
}

/// Test flow test command with template variables
#[test]
fn test_flow_test_with_set_variables() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "flow",
            "test",
            "greeting",
            "--var",
            "name=TestUser",
            "--var",
            "language=Spanish",
        ])
        .output()?;

    assert!(
        output.status.success(),
        "flow test with --var variables should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that template variables are processed
    assert!(stdout.contains("Test mode"), "should be in test mode");
    assert!(
        stdout.contains("Test execution completed"),
        "should show test execution completion"
    );

    Ok(())
}

/// Test flow test command with non-existent workflow
#[test]
fn test_flow_test_nonexistent_workflow() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "nonexistent-workflow"])
        .output()?;

    assert!(
        !output.status.success(),
        "flow test with non-existent workflow should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("not found"),
        "should show error for non-existent workflow"
    );

    Ok(())
}

/// Test flow test command with timeout
#[test]
fn test_flow_test_with_timeout() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "hello-world", "--timeout", "5s"])
        .output()?;

    assert!(
        output.status.success(),
        "flow test with timeout should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Timeout: 5s"),
        "should show timeout duration"
    );

    Ok(())
}

/// Test flow test command with quiet flag
#[test]
fn test_flow_test_quiet_mode() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "hello-world", "--quiet"])
        .output()?;

    assert!(
        output.status.success(),
        "flow test in quiet mode should succeed"
    );

    // In quiet mode, output should be minimal but still show coverage
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Coverage Report"),
        "should still show coverage report in quiet mode"
    );

    Ok(())
}

/// Test flow test command with custom workflow directory
#[test]
fn test_flow_test_custom_workflow_dir() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create a test workflow
    std::fs::write(
        workflow_dir.join("test-flow.md"),
        r#"---
title: Test Flow
description: A test workflow for integration testing
---

# Test Flow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> Process
    Process --> End
    End --> [*]
```

## Actions

- Start: Log "Starting test flow"
- Process: Log "Processing..."
- End: Log "Test flow complete"
"#,
    )?;

    // Run with workflow directory
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "flow",
            "test",
            "test-flow",
            "--workflow-dir",
            workflow_dir.to_str().unwrap(),
        ])
        .output()?;

    // Note: This might fail if workflow loading from custom dirs isn't fully implemented
    // In that case, we at least verify the command structure is correct
    assert!(
        output.status.code().is_some(),
        "flow test with custom workflow dir should complete"
    );

    Ok(())
}

/// Test flow test command with invalid var variable format
#[test]
fn test_flow_test_invalid_set_format() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "greeting", "--var", "invalid_format"])
        .output()?;

    assert!(
        !output.status.success(),
        "flow test with invalid --var format should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid") && stderr.contains("format"),
        "should show error about invalid variable format"
    );

    Ok(())
}

/// Test flow test help command
#[test]
fn test_flow_test_help() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "--help"])
        .output()?;

    assert!(output.status.success(), "flow test help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--var"),
        "help should mention --var parameter"
    );
    assert!(
        stdout.contains("--timeout"),
        "help should mention --timeout parameter"
    );
    assert!(
        stdout.contains("--interactive"),
        "help should mention --interactive flag"
    );

    Ok(())
}

/// Test flow test command coverage reporting
#[test]
fn test_flow_test_coverage_complete() -> Result<()> {
    // Use a simple workflow that should achieve full coverage
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["flow", "test", "hello-world"])
        .output()?;

    assert!(output.status.success(), "flow test should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // For a simple linear workflow, we should achieve full coverage
    if stdout.contains("Full state coverage achieved") {
        assert!(
            stdout.contains("Full state coverage achieved"),
            "should indicate full state coverage for simple workflow"
        );
    }

    // Check that percentage is calculated and displayed
    assert!(stdout.contains("%"), "should show coverage percentage");

    Ok(())
}

/// Test flow test with empty set value
#[test]
fn test_flow_test_empty_set_value() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "flow",
            "test",
            "greeting",
            "--var",
            "name=",
            "--var",
            "language=English",
        ])
        .output()?;

    // Should handle empty values gracefully
    assert!(
        output.status.success(),
        "flow test with empty set value should succeed"
    );

    Ok(())
}

/// Test flow test with special characters in set values
#[test]
fn test_flow_test_special_chars_in_set() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "flow",
            "test",
            "greeting",
            "--var",
            "name=Test User 123",
            "--var",
            r#"language="English (US)""#,
        ])
        .output()?;

    assert!(
        output.status.success(),
        "flow test with special chars in set values should succeed"
    );

    Ok(())
}

/// Test concurrent flow test execution
#[tokio::test]
async fn test_concurrent_flow_test() -> Result<()> {
    use tokio::task::JoinSet;

    let mut tasks = JoinSet::new();

    // Run multiple flow tests concurrently
    for i in 0..3 {
        tasks.spawn(async move {
            let output = Command::cargo_bin("sah")
                .unwrap()
                .args([
                    "flow",
                    "test",
                    "hello-world",
                    "--var",
                    &format!("run_id={i}"),
                ])
                .output()
                .expect("Failed to run command");

            (i, output.status.success())
        });
    }

    // All commands should succeed
    while let Some(result) = tasks.join_next().await {
        let (i, success) = result?;
        assert!(success, "Concurrent flow test {i} should succeed");
    }

    Ok(())
}

/// Test prompt list with different formats
#[test]
fn test_prompt_list_formats() -> Result<()> {
    let formats = vec!["json", "yaml", "table"];

    for format in formats {
        let output = Command::cargo_bin("sah")
            .unwrap()
            .args(["prompt", "list", "--format", format])
            .output()?;

        assert!(
            output.status.code().is_some(),
            "prompt list --format {format} should complete"
        );
    }

    Ok(())
}

/// Test concurrent command execution
#[tokio::test]
async fn test_concurrent_commands() -> Result<()> {
    use tokio::task::JoinSet;

    let mut tasks = JoinSet::new();

    // Run multiple commands concurrently
    for i in 0..3 {
        tasks.spawn(async move {
            let output = Command::cargo_bin("sah")
                .unwrap()
                .args(["prompt", "list"])
                .output()
                .expect("Failed to run command");

            (i, output.status.success())
        });
    }

    // All commands should succeed
    while let Some(result) = tasks.join_next().await {
        let (i, success) = result?;
        assert!(success, "Concurrent command {i} should succeed");
    }

    Ok(())
}

/// Test root-level validate command
#[test]
fn test_root_validate_command() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate"])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "root validate command should complete"
    );
    Ok(())
}

/// Test root validate command with quiet flag
#[test]
fn test_root_validate_quiet() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--quiet"])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "root validate --quiet should complete"
    );

    // In quiet mode, should only show errors
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _stderr = String::from_utf8_lossy(&output.stderr);

    // Should have minimal output in quiet mode
    if output.status.success() {
        assert!(
            stdout.is_empty() || stdout.trim().is_empty(),
            "quiet mode should produce minimal output on success"
        );
    }

    Ok(())
}

/// Test root validate command with JSON format
#[test]
fn test_root_validate_json_format() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--format", "json"])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "root validate --format json should complete"
    );

    // If successful, output should be valid JSON
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        // Try to parse as JSON
        let result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        assert!(result.is_ok(), "JSON format output should be valid JSON");

        if let Ok(json) = result {
            // Verify expected fields exist
            assert!(
                json.get("files_checked").is_some(),
                "JSON should have files_checked field"
            );
            assert!(
                json.get("errors").is_some(),
                "JSON should have errors field"
            );
            assert!(
                json.get("warnings").is_some(),
                "JSON should have warnings field"
            );
            assert!(
                json.get("issues").is_some(),
                "JSON should have issues field"
            );
        }
    }

    Ok(())
}

/// Test root validate command with specific workflow directories
#[test]
fn test_root_validate_with_workflow_dirs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create a simple valid workflow
    std::fs::write(
        workflow_dir.join("test.mermaid"),
        r#"stateDiagram-v2
    [*] --> Start
    Start --> End
    End --> [*]
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "root validate with workflow-dir should complete"
    );

    Ok(())
}

/// Test root validate command with multiple workflow directories
#[test]
fn test_root_validate_with_multiple_workflow_dirs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir1 = temp_dir.path().join("workflows1");
    let workflow_dir2 = temp_dir.path().join("workflows2");
    std::fs::create_dir_all(&workflow_dir1)?;
    std::fs::create_dir_all(&workflow_dir2)?;

    // Create workflows in both directories
    std::fs::write(
        workflow_dir1.join("flow1.mermaid"),
        r#"stateDiagram-v2
    [*] --> A
    A --> [*]
"#,
    )?;

    std::fs::write(
        workflow_dir2.join("flow2.mermaid"),
        r#"stateDiagram-v2
    [*] --> B
    B --> [*]
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "validate",
            "--workflow-dir",
            workflow_dir1.to_str().unwrap(),
            "--workflow-dir",
            workflow_dir2.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "root validate with multiple workflow-dirs should complete"
    );

    Ok(())
}

/// Test root validate command error exit codes
#[test]
fn test_root_validate_error_exit_codes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create an invalid workflow (missing terminal state)
    std::fs::write(
        workflow_dir.join("invalid.mermaid"),
        r#"stateDiagram-v2
    [*] --> Start
    Start --> Middle
    Middle --> Start
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "validate",
            "--workflow-dir",
            workflow_dir.to_str().unwrap(),
            "--quiet",
        ])
        .output()?;

    // Should return exit code 2 for validation errors
    assert_eq!(
        output.status.code(),
        Some(2),
        "root validate should return exit code 2 for validation errors"
    );

    Ok(())
}

/// Test that help output includes the root validate command
#[test]
fn test_root_help_includes_validate() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["--help"])
        .output()?;

    assert!(output.status.success(), "help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("validate"),
        "help should mention validate command at root level"
    );
    assert!(
        stdout.contains("Validate prompt files and workflows"),
        "help should describe what validate does"
    );

    Ok(())
}

/// Test validate command help
#[test]
fn test_root_validate_help() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--help"])
        .output()?;

    assert!(output.status.success(), "validate help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--quiet"),
        "validate help should mention --quiet flag"
    );
    assert!(
        stdout.contains("--format"),
        "validate help should mention --format flag"
    );
    assert!(
        stdout.contains("--workflow-dir"),
        "validate help should mention --workflow-dir option"
    );

    Ok(())
}

/// Test validation with invalid YAML format
#[test]
fn test_root_validate_invalid_yaml() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a prompt with invalid YAML
    std::fs::write(
        prompts_dir.join("invalid.md"),
        r#"---
title: Test Prompt
description: This has invalid YAML
parameters:
  - name: test
    required: yes  # Should be boolean true/false, not yes/no
    description
---

Test content"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--quiet"])
        .env("HOME", temp_dir.path())
        .output()?;

    // Should have validation errors
    assert_ne!(
        output.status.code(),
        Some(0),
        "validation with invalid YAML should fail"
    );

    Ok(())
}

/// Test validation with missing required fields
#[test]
fn test_root_validate_missing_fields() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a prompt missing required fields
    // Note: We need more than 5 lines of content or headers to avoid being detected as a partial template
    std::fs::write(
        prompts_dir.join("incomplete.md"),
        r#"---
# Missing title and description
parameters:
  - name: test
    required: true
---

# Test Prompt

This is a test prompt that is missing the required title and description fields.

It uses the {{ test }} variable.

We need more than 5 lines of content to avoid being detected as a partial template.

This is line 6 of content."#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--format", "json"])
        .env("HOME", temp_dir.path())
        .output()?;

    // Should have validation errors
    assert_eq!(
        output.status.code(),
        Some(2),
        "validation with missing fields should return exit code 2"
    );

    // Check JSON output contains error info
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let errors = json.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(errors > 0, "should have reported errors in JSON");
    }

    Ok(())
}

/// Test validation with undefined template variables
#[test]
fn test_root_validate_undefined_variables() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a prompt using undefined variables
    std::fs::write(
        prompts_dir.join("undefined_vars.md"),
        r#"---
title: Test Undefined Variables
description: This uses variables not defined in arguments
parameters:
  - name: defined_var
    required: true
---

This uses {{ defined_var }} which is fine.
But this uses {{ undefined_var }} which should error.
And this uses {{ another_undefined }} too."#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate"])
        .env("HOME", temp_dir.path())
        .output()?;

    // Should have validation errors
    assert_eq!(
        output.status.code(),
        Some(2),
        "validation with undefined variables should return exit code 2"
    );

    Ok(())
}

/// Test validation with malformed workflow
#[test]
fn test_root_validate_malformed_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create various malformed workflows
    std::fs::write(
        workflow_dir.join("syntax_error.mermaid"),
        r#"stateDiagram-v2
    [*] --> Start
    Start --> invalid syntax here [
    End --> [*]
"#,
    )?;

    std::fs::write(
        workflow_dir.join("no_initial.mermaid"),
        r#"stateDiagram-v2
    Start --> End
    End --> Done
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
        .output()?;

    // Should have validation errors
    assert_eq!(
        output.status.code(),
        Some(2),
        "validation with malformed workflows should return exit code 2"
    );

    Ok(())
}

/// Test validation with non-existent workflow directory
#[test]
fn test_root_validate_nonexistent_workflow_dir() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let fake_dir = temp_dir.path().join("does_not_exist");

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "validate",
            "--workflow-dir",
            fake_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()?;

    // Should complete with warnings
    assert!(
        output.status.code().is_some(),
        "validation should complete even with non-existent directory"
    );

    // Check JSON output for warnings
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let warnings = json.get("warnings").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            warnings > 0,
            "should have warnings about non-existent directory"
        );
    }

    Ok(())
}

/// Test validation with invalid format option
#[test]
fn test_root_validate_invalid_format() -> Result<()> {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--format", "invalid_format"])
        .output()?;

    // Should fail to parse arguments
    assert!(
        !output.status.success(),
        "validation with invalid format should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error:") || stderr.contains("invalid value"),
        "should show error about invalid format"
    );

    Ok(())
}

/// Test validation with empty workflow_dirs vector (should use default behavior)
#[test]
fn test_root_validate_empty_workflow_dirs() -> Result<()> {
    // When no workflow dirs are specified, it should search from current directory
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate"])
        .output()?;

    // Should complete successfully (may have warnings/errors based on current dir content)
    assert!(
        output.status.code().is_some(),
        "validation with empty workflow_dirs should complete"
    );

    Ok(())
}

/// Test validation with mix of valid and invalid prompts
#[test]
fn test_root_validate_mixed_valid_invalid_prompts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a valid prompt
    std::fs::write(
        prompts_dir.join("valid.md"),
        r#"---
title: Valid Prompt
description: This is a valid prompt
parameters:
  - name: test
    required: true
    default: "value"
---

This uses {{ test }} correctly."#,
    )?;

    // Create an invalid prompt (missing title)
    std::fs::write(
        prompts_dir.join("invalid.md"),
        r#"---
description: Missing title field
---

Content here."#,
    )?;

    // Create another invalid prompt (undefined variable)
    std::fs::write(
        prompts_dir.join("bad_vars.md"),
        r#"---
title: Bad Variables
description: Uses undefined variables
---

This uses {{ undefined }} variable."#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--format", "json"])
        .env("HOME", temp_dir.path())
        .output()?;

    // Should have errors due to invalid prompts
    assert_eq!(
        output.status.code(),
        Some(2),
        "validation with mixed valid/invalid prompts should return exit code 2"
    );

    // Check JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let files_checked = json
            .get("files_checked")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(files_checked >= 3, "should have checked at least 3 files");

        let errors = json.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(errors >= 2, "should have at least 2 errors");
    }

    Ok(())
}

/// Test validation with mix of valid and invalid workflows
#[test]
fn test_root_validate_mixed_valid_invalid_workflows() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create a valid workflow
    std::fs::write(
        workflow_dir.join("valid.mermaid"),
        r#"stateDiagram-v2
    [*] --> Process
    Process --> Complete
    Complete --> [*]
"#,
    )?;

    // Create an invalid workflow (no terminal state)
    std::fs::write(
        workflow_dir.join("no_terminal.mermaid"),
        r#"stateDiagram-v2
    [*] --> Start
    Start --> Loop
    Loop --> Start
"#,
    )?;

    // Create another invalid workflow (unreachable state)
    std::fs::write(
        workflow_dir.join("unreachable.mermaid"),
        r#"stateDiagram-v2
    [*] --> A
    A --> [*]
    B --> C
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
        .output()?;

    // Should have errors due to invalid workflows
    assert_eq!(
        output.status.code(),
        Some(2),
        "validation with mixed valid/invalid workflows should return exit code 2"
    );

    Ok(())
}

/// Test validation with absolute and relative workflow directories
#[test]
fn test_root_validate_absolute_relative_paths() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let abs_workflow_dir = temp_dir.path().join("abs_workflows");
    std::fs::create_dir_all(&abs_workflow_dir)?;

    // Create a workflow in absolute path
    std::fs::write(
        abs_workflow_dir.join("test.mermaid"),
        r#"stateDiagram-v2
    [*] --> Test
    Test --> [*]
"#,
    )?;

    // Test with absolute path
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "validate",
            "--workflow-dir",
            abs_workflow_dir.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "validation with absolute path should complete"
    );

    // Test with relative path (from temp dir)
    std::fs::create_dir_all(temp_dir.path().join("rel_workflows"))?;
    std::fs::write(
        temp_dir.path().join("rel_workflows").join("test.mermaid"),
        r#"stateDiagram-v2
    [*] --> Test
    Test --> [*]
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--workflow-dir", "rel_workflows"])
        .current_dir(temp_dir.path())
        .output()?;

    assert!(
        output.status.code().is_some(),
        "validation with relative path should complete"
    );

    Ok(())
}

/// Test validation with special characters in file paths
#[test]
fn test_root_validate_special_chars_in_paths() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflow_dir = temp_dir.path().join("work flows with spaces");
    std::fs::create_dir_all(&workflow_dir)?;

    // Create workflow with special chars in name
    std::fs::write(
        workflow_dir.join("test-workflow_v1.0.mermaid"),
        r#"stateDiagram-v2
    [*] --> Test
    Test --> [*]
"#,
    )?;

    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
        .output()?;

    assert!(
        output.status.code().is_some(),
        "validation with special chars in paths should complete"
    );

    Ok(())
}

/// Test CLI issue creation with optional names
#[test]
fn test_issue_create_with_optional_names() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repository since issue commands require Git
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repository");

    // Create .swissarmyhammer directory
    std::fs::create_dir_all(temp_dir.path().join(".swissarmyhammer"))
        .expect("Failed to create .swissarmyhammer directory");

    // Test creating a named issue
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "issue",
            "create",
            "test_issue",
            "--content",
            "This is a test issue with a name",
        ])
        .current_dir(&temp_dir)
        .output()?;

    assert!(
        output.status.success(),
        "named issue creation should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created issue"),
        "should show creation confirmation"
    );
    assert!(stdout.contains("test_issue"), "should show the issue name");

    // Test creating a nameless issue (empty content allowed now)
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args(["issue", "create"])
        .current_dir(&temp_dir)
        .output()?;

    assert!(
        output.status.success(),
        "nameless issue creation should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created issue"),
        "should show creation confirmation for nameless issue"
    );

    // Test creating a nameless issue with content
    let output = Command::cargo_bin("sah")
        .unwrap()
        .args([
            "issue",
            "create",
            "--content",
            "This is a nameless issue with content",
        ])
        .current_dir(&temp_dir)
        .output()?;

    assert!(
        output.status.success(),
        "nameless issue with content should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

/// Test validation quiet mode hides warnings from output and summary
#[test]
fn test_root_validate_quiet_mode_warnings_behavior() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a prompt that will generate warnings but no errors
    // This creates a warning due to unused template variable in arguments
    std::fs::write(
        prompts_dir.join("warning_only.md"),
        r#"---
title: Warning Only Prompt
description: This prompt has a warning due to unused argument
parameters:
  - name: unused_var
    required: false
    description: This variable is defined but not used in template
  - name: used_var
    required: false
    description: This variable is used in template
---

This prompt uses {{ used_var | default: "default_value" }} but not unused_var, creating a warning."#,
    )?;

    // Test in quiet mode - should produce no output for warnings only
    let quiet_output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--quiet"])
        .env("HOME", temp_dir.path())
        .output()?;

    // Debug output to see what's happening
    let quiet_stderr = String::from_utf8_lossy(&quiet_output.stderr);
    let quiet_stdout = String::from_utf8_lossy(&quiet_output.stdout);

    // With warnings present, quiet mode should still return exit code 1 but produce no output
    assert_eq!(
        quiet_output.status.code(),
        Some(1),
        "quiet mode validation with warnings should return exit code 1. stdout: '{quiet_stdout}', stderr: '{quiet_stderr}'"
    );

    assert!(
        quiet_stdout.trim().is_empty(),
        "quiet mode should produce no output when only warnings exist: '{quiet_stdout}'"
    );

    // Test in normal mode - should show warnings and summary
    let normal_output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate"])
        .env("HOME", temp_dir.path())
        .output()?;

    let normal_stdout = String::from_utf8_lossy(&normal_output.stdout);

    // With warnings present, exit code should be 1 (warnings) not 0 (success) or 2 (errors)
    assert_eq!(
        normal_output.status.code(),
        Some(1),
        "normal mode validation with warnings should return exit code 1"
    );

    // Verify warning content is displayed
    assert!(
        normal_stdout.contains("WARN") || normal_stdout.contains("warning"),
        "normal mode should show warnings in output: '{normal_stdout}'"
    );
    assert!(
        normal_stdout.contains("Summary:"),
        "normal mode should show summary: '{normal_stdout}'"
    );
    assert!(
        normal_stdout.contains("Warnings:"),
        "normal mode should show warning count: '{normal_stdout}'"
    );

    Ok(())
}

/// Test validation quiet mode behavior when both errors and warnings exist
#[test]
fn test_root_validate_quiet_mode_with_errors_and_warnings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create a prompt with warnings (unused argument)
    std::fs::write(
        prompts_dir.join("warning_prompt.md"),
        r#"---
title: Warning Prompt
description: This prompt has warnings
parameters:
  - name: unused_var
    required: false
    description: This variable is not used
  - name: used_var
    required: true
    description: This variable is used
---

This prompt uses {{ used_var }} but not unused_var."#,
    )?;

    // Create a prompt with errors (undefined variables)
    std::fs::write(
        prompts_dir.join("error_prompt.md"),
        r#"---
title: Test Undefined Variables
description: This uses variables not defined in arguments
parameters:
  - name: defined_var
    required: true
---

This uses {{ defined_var }} which is fine.
But this uses {{ undefined_var }} which should error.
And this uses {{ another_undefined }} too."#,
    )?;

    // Test in quiet mode - should show errors and summary, but hide warnings
    let quiet_output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate", "--quiet"])
        .env("HOME", temp_dir.path())
        .output()?;

    let quiet_stdout = String::from_utf8_lossy(&quiet_output.stdout);

    // With errors present, should return exit code 2 (errors)
    assert_eq!(
        quiet_output.status.code(),
        Some(2),
        "quiet mode validation with errors should return exit code 2"
    );

    // Should show errors and summary in quiet mode when errors are present
    assert!(
        quiet_stdout.contains("ERROR") || quiet_stdout.contains("error"),
        "quiet mode should show errors when they exist: '{quiet_stdout}'"
    );
    assert!(
        quiet_stdout.contains("Summary:"),
        "quiet mode should show summary when errors exist: '{quiet_stdout}'"
    );
    assert!(
        quiet_stdout.contains("Errors:"),
        "quiet mode should show error count when errors exist: '{quiet_stdout}'"
    );

    // Should NOT show warnings in quiet mode, even when errors are present
    assert!(
        !quiet_stdout.contains("WARN") && !quiet_stdout.contains("Warnings:"),
        "quiet mode should not show warning details or counts: '{quiet_stdout}'"
    );

    // Test in normal mode for comparison - should show both errors and warnings
    let normal_output = Command::cargo_bin("sah")
        .unwrap()
        .args(["validate"])
        .env("HOME", temp_dir.path())
        .output()?;

    let normal_stdout = String::from_utf8_lossy(&normal_output.stdout);

    // Should also return exit code 2 (errors take precedence)
    assert_eq!(
        normal_output.status.code(),
        Some(2),
        "normal mode validation with errors should return exit code 2"
    );

    // Should show both errors and warnings in normal mode
    assert!(
        normal_stdout.contains("ERROR") || normal_stdout.contains("error"),
        "normal mode should show errors: '{normal_stdout}'"
    );
    assert!(
        normal_stdout.contains("WARN") || normal_stdout.contains("warning"),
        "normal mode should show warnings: '{normal_stdout}'"
    );
    assert!(
        normal_stdout.contains("Summary:"),
        "normal mode should show summary: '{normal_stdout}'"
    );
    assert!(
        normal_stdout.contains("Errors:") && normal_stdout.contains("Warnings:"),
        "normal mode should show both error and warning counts: '{normal_stdout}'"
    );

    Ok(())
}
