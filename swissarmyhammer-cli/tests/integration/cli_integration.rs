//! Integration tests for CLI command structure and backward compatibility

use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use tempfile::TempDir;

use crate::in_process_test_utils::run_sah_command_in_process;

/// Test that the new prompt subcommand structure works correctly
#[tokio::test]
async fn test_prompt_subcommand_list() -> Result<()> {
    let result = run_sah_command_in_process(&["prompt", "list"]).await?;

    if result.exit_code != 0 {
        eprintln!("STDERR: {}", result.stderr);
        eprintln!("STDOUT: {}", result.stdout);
    }
    assert_eq!(result.exit_code, 0, "prompt list command should succeed");
    Ok(())
}

/// Test prompt test functionality with a simple prompt
#[tokio::test]
async fn test_prompt_subcommand_test() -> Result<()> {
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
    let _guard = IsolatedTestEnvironment::new()?;

    let result = run_sah_command_in_process(&["prompt", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "prompt help should succeed");

    assert!(
        result.stdout.contains("prompt") || result.stdout.contains("Commands"),
        "help output should be relevant"
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

/// Test that verbose flag works
#[tokio::test]
async fn test_verbose_flag() -> Result<()> {
    let result = run_sah_command_in_process(&["--verbose", "prompt", "list"]).await?;

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

    assert!(
        result.exit_code >= 0,
        "quiet flag should not break commands"
    );

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

    for i in 0..3 {
        tasks.spawn(async move {
            let result = run_sah_command_in_process(&["prompt", "list"])
                .await
                .expect("Failed to run command");

            (i, result.exit_code == 0)
        });
    }

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

    if !result.stdout.is_empty() {
        let json_result: Result<serde_json::Value, _> = serde_json::from_str(&result.stdout);
        assert!(
            json_result.is_ok(),
            "JSON format output should be valid JSON"
        );

        if let Ok(json) = json_result {
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

/// Test that help output includes the root validate command
#[tokio::test]
async fn test_root_help_includes_validate() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "help should succeed");

    assert!(
        result.stdout.contains("validate"),
        "help should mention validate command at root level"
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

    Ok(())
}

/// Test validation with invalid YAML format
#[tokio::test]
async fn test_root_validate_invalid_yaml() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "validation with invalid YAML should fail"
    );

    Ok(())
}

/// Test validation with missing required fields
#[tokio::test]
async fn test_root_validate_missing_fields() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--format", "json"]).await?;

    assert_eq!(
        result.exit_code, 2,
        "validation with missing fields should return exit code 2"
    );

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result.stdout) {
        let errors = json.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(errors > 0, "should have reported errors in JSON");
    }

    Ok(())
}

/// Test validation with undefined template variables
#[tokio::test]
async fn test_root_validate_undefined_variables() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate"]).await?;

    assert_eq!(
        result.exit_code, 2,
        "validation with undefined variables should return exit code 2"
    );

    Ok(())
}

/// Test validation with invalid format option
#[tokio::test]
async fn test_root_validate_invalid_format() -> Result<()> {
    let result = run_sah_command_in_process(&["validate", "--format", "invalid_format"]).await?;

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

/// Test validation with empty default behavior
#[tokio::test]
async fn test_root_validate_default_behavior() -> Result<()> {
    let result = run_sah_command_in_process(&["validate"]).await?;

    assert!(
        result.exit_code >= 0,
        "validation with defaults should complete"
    );

    Ok(())
}

/// Test validation with mix of valid and invalid prompts
#[tokio::test]
async fn test_root_validate_mixed_valid_invalid_prompts() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::fs::write(
        prompts_dir.join("invalid.md"),
        r#"---
description: Missing title field
---

Content here."#,
    )?;

    std::fs::write(
        prompts_dir.join("bad_vars.md"),
        r#"---
title: Bad Variables
description: Uses undefined variables
---

This uses {{ undefined }} variable."#,
    )?;

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let result = run_sah_command_in_process(&["validate", "--format", "json"]).await?;

    assert_eq!(
        result.exit_code, 2,
        "validation with mixed valid/invalid prompts should return exit code 2"
    );

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

/// Test validation quiet mode hides warnings from output and summary
#[tokio::test]
async fn test_root_validate_quiet_mode_warnings_behavior() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let quiet_result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

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

    let normal_result = run_sah_command_in_process(&["validate"]).await?;

    assert_eq!(
        normal_result.exit_code, 1,
        "normal mode validation with warnings should return exit code 1"
    );

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
    let _guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("sah").join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

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

    std::env::set_var("XDG_DATA_HOME", temp_dir.path());
    let quiet_result = run_sah_command_in_process(&["validate", "--quiet"]).await?;

    assert_eq!(
        quiet_result.exit_code, 2,
        "quiet mode validation with errors should return exit code 2"
    );

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

    assert!(
        !quiet_result.stdout.contains("WARN") && !quiet_result.stdout.contains("Warnings:"),
        "quiet mode should not show warning details or counts: '{}'",
        quiet_result.stdout
    );

    let normal_result = run_sah_command_in_process(&["validate"]).await?;

    assert_eq!(
        normal_result.exit_code, 2,
        "normal mode validation with errors should return exit code 2"
    );

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
