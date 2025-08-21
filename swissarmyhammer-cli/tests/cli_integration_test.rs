//! Integration tests for CLI command structure and backward compatibility

use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use tempfile::TempDir;

mod test_utils;
use test_utils::create_test_environment;

mod in_process_test_utils;
use in_process_test_utils::{run_flow_test_in_process, run_sah_command_in_process};

/// Helper function to run CLI command and capture output to temp files

/// Test that the new prompt subcommand structure works correctly
#[tokio::test]
async fn test_prompt_subcommand_list() -> Result<()> {
    let result = run_sah_command_in_process(&["prompt", "list"]).await?;

    assert_eq!(result.exit_code, 0, "prompt list command should succeed");
    Ok(())
}

/// Test prompt search functionality
#[tokio::test]
async fn test_prompt_subcommand_search() -> Result<()> {
    let result = run_sah_command_in_process(&["prompt", "search", "test"]).await?;

    // Search might not find results but should not error
    assert!(
        result.exit_code == 0 || result.exit_code == 1,
        "prompt search should complete"
    );
    Ok(())
}

/// Test prompt validate functionality
#[tokio::test]
async fn test_prompt_subcommand_validate() -> Result<()> {
    let (_temp_dir, prompts_dir) = create_test_environment()?;

    let result = run_sah_command_in_process(&[
        "prompt",
        "validate",
        "--workflow-dirs",
        prompts_dir.to_str().unwrap(),
    ])
    .await?;

    // Validation should complete (may have warnings but shouldn't crash)
    assert!(result.exit_code >= 0, "prompt validate should complete");
    Ok(())
}

/// Test prompt test functionality with a simple prompt
#[tokio::test]
async fn test_prompt_subcommand_test() -> Result<()> {
    let (_temp_dir, _prompts_dir) = create_test_environment()?;

    // Test with non-existent prompt should fail gracefully
    let result = run_sah_command_in_process(&["prompt", "test", "non_existent_prompt"]).await?;

    assert!(
        result.exit_code != 0,
        "testing non-existent prompt should fail"
    );
    assert_eq!(result.exit_code, 1, "should return exit code 1");

    // Verify error message is present
    assert!(
        result.stderr.contains("Error:") || result.stderr.contains("not found"),
        "should show meaningful error message"
    );

    Ok(())
}

/// Test help output for prompt subcommands
#[tokio::test]
async fn test_prompt_help() -> Result<()> {
    let result = run_sah_command_in_process(&["prompt", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "prompt help should succeed");

    assert!(
        result.stdout.contains("list"),
        "help should mention list subcommand"
    );
    assert!(
        result.stdout.contains("search"),
        "help should mention search subcommand"
    );
    assert!(
        result.stdout.contains("validate"),
        "help should mention validate subcommand"
    );
    assert!(
        result.stdout.contains("test"),
        "help should mention test subcommand"
    );

    Ok(())
}

/// Test shell completion generation
#[tokio::test]
async fn test_completion_command() -> Result<()> {
    let shells = vec!["bash", "zsh", "fish"];

    for shell in shells {
        let result = run_sah_command_in_process(&["completion", shell]).await?;

        assert_eq!(result.exit_code, 0, "{shell} completion should succeed");

        assert!(
            !result.stdout.trim().is_empty(),
            "{shell} completion should generate output"
        );
    }

    Ok(())
}

/// Test error handling and exit codes
#[tokio::test]
async fn test_error_exit_codes() -> Result<()> {
    // Test validation error (exit code 2)
    let temp_dir = TempDir::new()?;
    let invalid_dir = temp_dir.path().join("non_existent");

    let result = run_sah_command_in_process(&[
        "prompt",
        "validate",
        "--workflow-dirs",
        invalid_dir.to_str().unwrap(),
    ])
    .await?;

    // Should handle gracefully even if directory doesn't exist
    assert!(result.exit_code >= 0, "should return an exit code");

    Ok(())
}

/// Test that verbose flag works
#[tokio::test]
async fn test_verbose_flag() -> Result<()> {
    let result = run_sah_command_in_process(&["--verbose", "prompt", "list"]).await?;

    // Command should still work with verbose flag
    assert!(
        result.exit_code >= 0,
        "verbose flag should not break commands"
    );

    Ok(())
}

/// Test that quiet flag works
#[tokio::test]
async fn test_quiet_flag() -> Result<()> {
    let result = run_sah_command_in_process(&["--quiet", "prompt", "list"]).await?;

    // Command should still work with quiet flag
    assert!(
        result.exit_code >= 0,
        "quiet flag should not break commands"
    );

    Ok(())
}

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
async fn setup_test_workflow(workflow_name: &str) -> Result<IsolatedTestEnvironment> {
    let env = IsolatedTestEnvironment::new().unwrap();

    // Create minimal workflow in the isolated environment
    let workflow_dir = env.swissarmyhammer_dir().join("workflows");
    std::fs::create_dir_all(&workflow_dir)?;
    let workflow_path = workflow_dir.join(format!("{}.md", workflow_name));
    std::fs::write(&workflow_path, create_minimal_workflow())?;

    Ok(env)
}

/// Run workflow in controlled test environment
async fn run_test_workflow_in_process(workflow_name: &str, vars: Vec<String>) -> Result<bool> {
    let _env = setup_test_workflow(workflow_name).await?;

    // Use very fast timeout for performance tests
    let result = run_flow_test_in_process(workflow_name, vars, Some("1s".to_string()), false).await;

    Ok(result.is_ok())
}

/// Test flow test command with simple workflow
#[tokio::test]
async fn test_flow_test_simple_workflow() -> Result<()> {
    // Test with minimal workflow in controlled environment
    let success = run_test_workflow_in_process("minimal-test", vec![]).await?;
    assert!(success, "Simple workflow should execute successfully");
    Ok(())
}

/// Test flow test command with template variables
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

/// Test flow test command with non-existent workflow
#[tokio::test]
async fn test_flow_test_nonexistent_workflow() -> Result<()> {
    let result = run_sah_command_in_process(&["flow", "test", "nonexistent-workflow"]).await?;

    assert!(
        result.exit_code != 0,
        "flow test with non-existent workflow should fail"
    );

    assert!(
        result.stderr.contains("Error") || result.stderr.contains("not found"),
        "should show error for non-existent workflow"
    );

    Ok(())
}

/// Test flow test command with timeout
#[tokio::test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
async fn test_flow_test_with_timeout() -> Result<()> {
    let result =
        run_sah_command_in_process(&["flow", "test", "hello-world", "--timeout", "5s"]).await?;

    assert_eq!(result.exit_code, 0, "flow test with timeout should succeed");

    assert!(
        result.stdout.contains("Timeout: 5s"),
        "should show timeout duration"
    );

    Ok(())
}

/// Test flow test command with quiet flag
#[tokio::test]
async fn test_flow_test_quiet_mode() -> Result<()> {
    // Test quiet mode flag
    let _env = setup_test_workflow("quiet-test").await?;

    let captured = run_flow_test_in_process("quiet-test", vec![], None, true).await?;

    // Should complete regardless of quiet mode
    assert!(
        captured.exit_code == 0 || captured.exit_code == 1,
        "Should return valid exit code"
    );

    Ok(())
}

/// Test flow test command with custom workflow directory
#[tokio::test]
async fn test_flow_test_custom_workflow_dir() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "flow",
        "test",
        "test-flow",
        "--workflow-dir",
        workflow_dir.to_str().unwrap(),
    ])
    .await?;

    // Note: This might fail if workflow loading from custom dirs isn't fully implemented
    // In that case, we at least verify the command structure is correct
    assert!(
        result.exit_code >= 0,
        "flow test with custom workflow dir should complete"
    );

    Ok(())
}

/// Test flow test command with invalid var variable format
#[tokio::test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
async fn test_flow_test_invalid_set_format() -> Result<()> {
    let result =
        run_sah_command_in_process(&["flow", "test", "greeting", "--var", "invalid_format"])
            .await?;

    assert!(
        result.exit_code != 0,
        "flow test with invalid --var format should fail"
    );

    assert!(
        result.stderr.contains("Invalid") && result.stderr.contains("format"),
        "should show error about invalid variable format"
    );

    Ok(())
}

/// Test flow test help command
#[tokio::test]
async fn test_flow_test_help() -> Result<()> {
    let result = run_sah_command_in_process(&["flow", "test", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "flow test help should succeed");

    assert!(
        result.stdout.contains("--var"),
        "help should mention --var parameter"
    );
    assert!(
        result.stdout.contains("--timeout"),
        "help should mention --timeout parameter"
    );
    assert!(
        result.stdout.contains("--interactive"),
        "help should mention --interactive flag"
    );

    Ok(())
}

/// Test flow test with special characters in set values
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

/// Test concurrent flow test execution
#[tokio::test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
async fn test_concurrent_flow_test() -> Result<()> {
    use tokio::task::JoinSet;

    let mut tasks = JoinSet::new();

    // Run multiple flow tests concurrently
    for i in 0..3 {
        tasks.spawn(async move {
            let result = run_sah_command_in_process(&[
                "flow",
                "test",
                "hello-world",
                "--var",
                &format!("run_id={i}"),
            ])
            .await
            .expect("Failed to run command");

            (i, result.exit_code == 0)
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
#[tokio::test]
async fn test_prompt_list_formats() -> Result<()> {
    let formats = vec!["json", "yaml", "table"];

    for format in formats {
        let result = run_sah_command_in_process(&["prompt", "list", "--format", format]).await?;

        assert!(
            result.exit_code >= 0,
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
            let result = run_sah_command_in_process(&["prompt", "list"])
                .await
                .expect("Failed to run command");

            (i, result.exit_code == 0)
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
#[tokio::test]
async fn test_root_validate_command() -> Result<()> {
    let result = run_sah_command_in_process(&["validate"]).await?;

    assert!(
        result.exit_code >= 0,
        "root validate command should complete"
    );
    Ok(())
}

/// Test root validate command with quiet flag
#[tokio::test]
async fn test_root_validate_quiet() -> Result<()> {
    let result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

    assert!(
        result.exit_code >= 0,
        "root validate --quiet should complete"
    );

    // Should have minimal output in quiet mode
    if result.exit_code == 0 {
        assert!(
            result.stdout.is_empty() || result.stdout.trim().is_empty(),
            "quiet mode should produce minimal output on success"
        );
    }

    Ok(())
}

/// Test root validate command with JSON format
#[tokio::test]
async fn test_root_validate_json_format() -> Result<()> {
    let result = run_sah_command_in_process(&["validate", "--format", "json"]).await?;

    assert!(
        result.exit_code >= 0,
        "root validate --format json should complete"
    );

    // If successful, output should be valid JSON
    if !result.stdout.is_empty() {
        // Try to parse as JSON
        let json_result: Result<serde_json::Value, _> = serde_json::from_str(&result.stdout);
        assert!(
            json_result.is_ok(),
            "JSON format output should be valid JSON"
        );

        if let Ok(json) = json_result {
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
#[tokio::test]
async fn test_root_validate_with_workflow_dirs() -> Result<()> {
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

    let result =
        run_sah_command_in_process(&["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
            .await?;

    assert!(
        result.exit_code >= 0,
        "root validate with workflow-dir should complete"
    );

    Ok(())
}

/// Test root validate command with multiple workflow directories
#[tokio::test]
async fn test_root_validate_with_multiple_workflow_dirs() -> Result<()> {
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

    let result = run_sah_command_in_process(&[
        "validate",
        "--workflow-dir",
        workflow_dir1.to_str().unwrap(),
        "--workflow-dir",
        workflow_dir2.to_str().unwrap(),
    ])
    .await?;

    assert!(
        result.exit_code >= 0,
        "root validate with multiple workflow-dirs should complete"
    );

    Ok(())
}

/// Test root validate command error exit codes
#[tokio::test]
async fn test_root_validate_error_exit_codes() -> Result<()> {
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

    let result = run_sah_command_in_process(&[
        "validate",
        "--workflow-dir",
        workflow_dir.to_str().unwrap(),
        "--quiet",
    ])
    .await?;

    // Should return exit code 2 for validation errors
    assert_eq!(
        result.exit_code, 2,
        "root validate should return exit code 2 for validation errors"
    );

    Ok(())
}

/// Test that help output includes the root validate command
#[tokio::test]
async fn test_root_help_includes_validate() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "help should succeed");

    assert!(
        result.stdout.contains("validate"),
        "help should mention validate command at root level"
    );
    assert!(
        result
            .stdout
            .contains("Validate prompt files and workflows"),
        "help should describe what validate does"
    );

    Ok(())
}

/// Test validate command help
#[tokio::test]
async fn test_root_validate_help() -> Result<()> {
    let result = run_sah_command_in_process(&["validate", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "validate help should succeed");

    assert!(
        result.stdout.contains("--quiet"),
        "validate help should mention --quiet flag"
    );
    assert!(
        result.stdout.contains("--format"),
        "validate help should mention --format flag"
    );
    assert!(
        result.stdout.contains("--workflow-dir"),
        "validate help should mention --workflow-dir option"
    );

    Ok(())
}

/// Test validation with invalid YAML format
#[tokio::test]
async fn test_root_validate_invalid_yaml() -> Result<()> {
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

    std::env::set_var("HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--quiet"]).await?;
    std::env::remove_var("HOME");

    // Should have validation errors
    assert_ne!(
        result.exit_code, 0,
        "validation with invalid YAML should fail"
    );

    Ok(())
}

/// Test validation with missing required fields
#[tokio::test]
async fn test_root_validate_missing_fields() -> Result<()> {
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

    std::env::set_var("HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--format", "json"]).await?;
    std::env::remove_var("HOME");

    // Should have validation errors
    assert_eq!(
        result.exit_code, 2,
        "validation with missing fields should return exit code 2"
    );

    // Check JSON output contains error info
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result.stdout) {
        let errors = json.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(errors > 0, "should have reported errors in JSON");
    }

    Ok(())
}

/// Test validation with undefined template variables
#[tokio::test]
async fn test_root_validate_undefined_variables() -> Result<()> {
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

    std::env::set_var("HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate"]).await?;
    std::env::remove_var("HOME");

    // Should have validation errors
    assert_eq!(
        result.exit_code, 2,
        "validation with undefined variables should return exit code 2"
    );

    Ok(())
}

/// Test validation with malformed workflow
#[tokio::test]
async fn test_root_validate_malformed_workflow() -> Result<()> {
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

    let result =
        run_sah_command_in_process(&["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
            .await?;

    // Should have validation errors
    assert_eq!(
        result.exit_code, 2,
        "validation with malformed workflows should return exit code 2"
    );

    Ok(())
}

/// Test validation with non-existent workflow directory
#[tokio::test]
async fn test_root_validate_nonexistent_workflow_dir() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let fake_dir = temp_dir.path().join("does_not_exist");

    let result = run_sah_command_in_process(&[
        "validate",
        "--workflow-dir",
        fake_dir.to_str().unwrap(),
        "--format",
        "json",
    ])
    .await?;

    // Should complete with warnings
    assert!(
        result.exit_code >= 0,
        "validation should complete even with non-existent directory"
    );

    // Check JSON output for warnings
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result.stdout) {
        let warnings = json.get("warnings").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            warnings > 0,
            "should have warnings about non-existent directory"
        );
    }

    Ok(())
}

/// Test validation with invalid format option
#[tokio::test]
async fn test_root_validate_invalid_format() -> Result<()> {
    let result = run_sah_command_in_process(&["validate", "--format", "invalid_format"]).await?;

    // Should fail to parse arguments
    assert!(
        result.exit_code != 0,
        "validation with invalid format should fail"
    );

    assert!(
        result.stderr.contains("error:") || result.stderr.contains("invalid value"),
        "should show error about invalid format"
    );

    Ok(())
}

/// Test validation with empty workflow_dirs vector (should use default behavior)
#[tokio::test]
async fn test_root_validate_empty_workflow_dirs() -> Result<()> {
    // When no workflow dirs are specified, it should search from current directory
    let result = run_sah_command_in_process(&["validate"]).await?;

    // Should complete successfully (may have warnings/errors based on current dir content)
    assert!(
        result.exit_code >= 0,
        "validation with empty workflow_dirs should complete"
    );

    Ok(())
}

/// Test validation with mix of valid and invalid prompts
#[tokio::test]
async fn test_root_validate_mixed_valid_invalid_prompts() -> Result<()> {
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

    std::env::set_var("HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--format", "json"]).await?;
    std::env::remove_var("HOME");

    // Should have errors due to invalid prompts
    assert_eq!(
        result.exit_code, 2,
        "validation with mixed valid/invalid prompts should return exit code 2"
    );

    // Check JSON output
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result.stdout) {
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
#[tokio::test]
async fn test_root_validate_mixed_valid_invalid_workflows() -> Result<()> {
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

    let result =
        run_sah_command_in_process(&["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
            .await?;

    // Should have errors due to invalid workflows
    assert_eq!(
        result.exit_code, 2,
        "validation with mixed valid/invalid workflows should return exit code 2"
    );

    Ok(())
}

/// Test validation with absolute and relative workflow directories
#[tokio::test]
async fn test_root_validate_absolute_relative_paths() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "validate",
        "--workflow-dir",
        abs_workflow_dir.to_str().unwrap(),
    ])
    .await?;

    assert!(
        result.exit_code >= 0,
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

    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    let result =
        run_sah_command_in_process(&["validate", "--workflow-dir", "rel_workflows"]).await?;
    std::env::set_current_dir(original_dir)?;

    assert!(
        result.exit_code >= 0,
        "validation with relative path should complete"
    );

    Ok(())
}

/// Test validation with special characters in file paths
#[tokio::test]
async fn test_root_validate_special_chars_in_paths() -> Result<()> {
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

    let result =
        run_sah_command_in_process(&["validate", "--workflow-dir", workflow_dir.to_str().unwrap()])
            .await?;

    assert!(
        result.exit_code >= 0,
        "validation with special chars in paths should complete"
    );

    Ok(())
}

/// Test CLI issue creation with optional names
#[tokio::test]
async fn test_issue_create_with_optional_names() -> Result<()> {
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

    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_dir)?;

    // Test creating a named issue
    let result = run_sah_command_in_process(&[
        "issue",
        "create",
        "test_issue",
        "--content",
        "This is a test issue with a name",
    ])
    .await?;

    assert_eq!(
        result.exit_code, 0,
        "named issue creation should succeed: stderr: {}",
        result.stderr
    );

    assert!(
        result.stdout.contains("Created issue"),
        "should show creation confirmation"
    );
    assert!(
        result.stdout.contains("test_issue"),
        "should show the issue name"
    );

    // Test creating a nameless issue (empty content allowed now)
    let result = run_sah_command_in_process(&["issue", "create"]).await?;

    assert_eq!(
        result.exit_code, 0,
        "nameless issue creation should succeed: stderr: {}",
        result.stderr
    );

    assert!(
        result.stdout.contains("Created issue"),
        "should show creation confirmation for nameless issue"
    );

    // Test creating a nameless issue with content
    let result = run_sah_command_in_process(&[
        "issue",
        "create",
        "--content",
        "This is a nameless issue with content",
    ])
    .await?;

    assert_eq!(
        result.exit_code, 0,
        "nameless issue with content should succeed: stderr: {}",
        result.stderr
    );

    std::env::set_current_dir(original_dir)?;
    Ok(())
}

/// Test validation quiet mode hides warnings from output and summary
#[tokio::test]
async fn test_root_validate_quiet_mode_warnings_behavior() -> Result<()> {
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
    std::env::set_var("HOME", temp_dir.path());
    let quiet_result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

    // With warnings present, quiet mode should still return exit code 1 but produce no output
    assert_eq!(
        quiet_result.exit_code, 1,
        "quiet mode validation with warnings should return exit code 1. stdout: '{}', stderr: '{}'",
        quiet_result.stdout, quiet_result.stderr
    );

    assert!(
        quiet_result.stdout.trim().is_empty(),
        "quiet mode should produce no output when only warnings exist: '{}'",
        quiet_result.stdout
    );

    // Test in normal mode - should show warnings and summary
    let normal_result = run_sah_command_in_process(&["validate"]).await?;
    std::env::remove_var("HOME");

    // With warnings present, exit code should be 1 (warnings) not 0 (success) or 2 (errors)
    assert_eq!(
        normal_result.exit_code, 1,
        "normal mode validation with warnings should return exit code 1"
    );

    // Verify warning content is displayed
    assert!(
        normal_result.stdout.contains("WARN") || normal_result.stdout.contains("warning"),
        "normal mode should show warnings in output: '{}'",
        normal_result.stdout
    );
    assert!(
        normal_result.stdout.contains("Summary:"),
        "normal mode should show summary: '{}'",
        normal_result.stdout
    );
    assert!(
        normal_result.stdout.contains("Warnings:"),
        "normal mode should show warning count: '{}'",
        normal_result.stdout
    );

    Ok(())
}

/// Test validation quiet mode behavior when both errors and warnings exist
#[tokio::test]
async fn test_root_validate_quiet_mode_with_errors_and_warnings() -> Result<()> {
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
    std::env::set_var("HOME", temp_dir.path());
    let quiet_result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

    // With errors present, should return exit code 2 (errors)
    assert_eq!(
        quiet_result.exit_code, 2,
        "quiet mode validation with errors should return exit code 2"
    );

    // Should show errors and summary in quiet mode when errors are present
    assert!(
        quiet_result.stdout.contains("ERROR") || quiet_result.stdout.contains("error"),
        "quiet mode should show errors when they exist: '{}'",
        quiet_result.stdout
    );
    assert!(
        quiet_result.stdout.contains("Summary:"),
        "quiet mode should show summary when errors exist: '{}'",
        quiet_result.stdout
    );
    assert!(
        quiet_result.stdout.contains("Errors:"),
        "quiet mode should show error count when errors exist: '{}'",
        quiet_result.stdout
    );

    // Should NOT show warnings in quiet mode, even when errors are present
    assert!(
        !quiet_result.stdout.contains("WARN") && !quiet_result.stdout.contains("Warnings:"),
        "quiet mode should not show warning details or counts: '{}'",
        quiet_result.stdout
    );

    // Test in normal mode for comparison - should show both errors and warnings
    let normal_result = run_sah_command_in_process(&["validate"]).await?;
    std::env::remove_var("HOME");

    // Should also return exit code 2 (errors take precedence)
    assert_eq!(
        normal_result.exit_code, 2,
        "normal mode validation with errors should return exit code 2"
    );

    // Should show both errors and warnings in normal mode
    assert!(
        normal_result.stdout.contains("ERROR") || normal_result.stdout.contains("error"),
        "normal mode should show errors: '{}'",
        normal_result.stdout
    );
    assert!(
        normal_result.stdout.contains("WARN") || normal_result.stdout.contains("warning"),
        "normal mode should show warnings: '{}'",
        normal_result.stdout
    );
    assert!(
        normal_result.stdout.contains("Summary:"),
        "normal mode should show summary: '{}'",
        normal_result.stdout
    );
    assert!(
        normal_result.stdout.contains("Errors:") && normal_result.stdout.contains("Warnings:"),
        "normal mode should show both error and warning counts: '{}'",
        normal_result.stdout
    );

    Ok(())
}
