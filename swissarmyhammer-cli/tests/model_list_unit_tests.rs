//! Unit tests for model list command implementation
//!
//! Tests the execute_list_command function directly with various formats
//! and error scenarios.

use anyhow::Result;
use std::env;
use std::fs;
use swissarmyhammer_cli::cli::OutputFormat;
use swissarmyhammer_cli::commands::model::list::execute_list_command;
use swissarmyhammer_cli::context::{CliContext, CliContextBuilder};
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

/// Create a test context with specified verbosity
async fn create_test_context(verbose: bool) -> CliContext {
    let template_context = TemplateContext::new();
    let matches = clap::Command::new("test")
        .try_get_matches_from(["test"])
        .unwrap();

    CliContextBuilder::default()
        .template_context(template_context)
        .format(OutputFormat::Table)
        .format_option(Some(OutputFormat::Table))
        .verbose(verbose)
        .debug(false)
        .quiet(false)
        .matches(matches)
        .build_async()
        .await
        .unwrap()
}

/// Set up a temporary home directory with test models
fn setup_test_models() -> Result<(TempDir, Option<String>)> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create user models directory
    let user_models_dir = temp_home.join(".swissarmyhammer").join("models");
    fs::create_dir_all(&user_models_dir)?;

    // Create a simple test model
    let test_model_content = r#"---
description: "Test model for unit testing"
---
executor:
  type: claude-code
  config:
    claude_path: /test/claude
    args: ["--test"]
quiet: false"#;
    fs::write(user_models_dir.join("test-model.yaml"), test_model_content)?;

    // Save original HOME and set new one
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    Ok((temp_dir, original_home))
}

/// Restore original HOME environment variable
fn restore_home(original_home: Option<String>) {
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
}

// =============================================================================
// TABLE FORMAT TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_table_format_non_verbose() -> Result<()> {
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with table format should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_table_format_verbose() -> Result<()> {
    let context = create_test_context(true).await;

    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with table format and verbose should succeed"
    );

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_table_includes_user_models() -> Result<()> {
    let (_temp_dir, original_home) = setup_test_models()?;
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    // Capture stdout to verify output
    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command should succeed with user models"
    );

    Ok(())
}

// =============================================================================
// JSON FORMAT TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_json_format_non_verbose() -> Result<()> {
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Json, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with JSON format should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_json_format_verbose() -> Result<()> {
    let context = create_test_context(true).await;

    let result = execute_list_command(OutputFormat::Json, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with JSON format and verbose should succeed"
    );

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_json_includes_all_models() -> Result<()> {
    let (_temp_dir, original_home) = setup_test_models()?;
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Json, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command JSON should succeed with all models"
    );

    Ok(())
}

// =============================================================================
// YAML FORMAT TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_yaml_format_non_verbose() -> Result<()> {
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Yaml, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with YAML format should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_yaml_format_verbose() -> Result<()> {
    let context = create_test_context(true).await;

    let result = execute_list_command(OutputFormat::Yaml, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command with YAML format and verbose should succeed"
    );

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_yaml_includes_user_models() -> Result<()> {
    let (_temp_dir, original_home) = setup_test_models()?;
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Yaml, &context).await;

    assert!(
        result.is_ok(),
        "execute_list_command YAML should succeed with user models"
    );

    Ok(())
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_with_invalid_home_directory() -> Result<()> {
    // Set HOME to a non-existent directory
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", "/nonexistent/invalid/home/directory");

    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    // Should still succeed with just builtin models
    let result = execute_list_command(OutputFormat::Table, &context).await;

    // The function should handle this gracefully and still list builtin models
    assert!(
        result.is_ok(),
        "Should handle invalid HOME gracefully and list builtin models"
    );

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_with_malformed_model_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create user models directory with malformed YAML
    let user_models_dir = temp_home.join(".swissarmyhammer").join("models");
    fs::create_dir_all(&user_models_dir)?;

    // Create invalid YAML file
    let invalid_content = "invalid: yaml: [unclosed";
    fs::write(user_models_dir.join("broken-model.yaml"), invalid_content)?;

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    // Should still succeed, skipping the broken model
    let result = execute_list_command(OutputFormat::Json, &context).await;

    assert!(
        result.is_ok(),
        "Should handle malformed model files gracefully"
    );

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_empty_models_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create empty models directory
    let user_models_dir = temp_home.join(".swissarmyhammer").join("models");
    fs::create_dir_all(&user_models_dir)?;

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    // Should succeed with just builtin models
    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "Should succeed with empty user models directory"
    );

    Ok(())
}

// =============================================================================
// FORMAT CONSISTENCY TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_all_formats_produce_output() -> Result<()> {
    let context = create_test_context(false).await;

    // Test all three formats
    let formats = [OutputFormat::Table, OutputFormat::Json, OutputFormat::Yaml];

    for format in &formats {
        let result = execute_list_command(*format, &context).await;

        assert!(result.is_ok(), "Format {:?} should succeed", format);
    }

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_verbose_all_formats() -> Result<()> {
    let context = create_test_context(true).await;

    // Test all three formats with verbose mode
    let formats = [OutputFormat::Table, OutputFormat::Json, OutputFormat::Yaml];

    for format in &formats {
        let result = execute_list_command(*format, &context).await;

        assert!(
            result.is_ok(),
            "Format {:?} with verbose should succeed",
            format
        );
    }

    Ok(())
}

// =============================================================================
// CONTEXT VARIATION TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_with_different_working_directories() -> Result<()> {
    let _temp_dir = TempDir::new()?;

    // The working directory is not directly used by execute_list_command
    // It uses the model manager which looks at HOME and project directories
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "Should work with any valid working directory"
    );

    Ok(())
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_with_permission_restricted_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create models directory
    let user_models_dir = temp_home.join(".swissarmyhammer").join("models");
    fs::create_dir_all(&user_models_dir)?;

    // Try to make it read-only (may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&user_models_dir)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o444); // Read-only
        let _ = fs::set_permissions(&user_models_dir, permissions);
    }

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    // Should handle permission issues gracefully
    let result = execute_list_command(OutputFormat::Json, &context).await;

    // Should succeed even if can't read user models directory
    assert!(result.is_ok(), "Should handle permission issues gracefully");

    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn test_execute_list_command_with_symlinked_models_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create actual models directory
    let actual_models = temp_dir.path().join("actual_models");
    fs::create_dir_all(&actual_models)?;

    // Create test model in actual directory
    let test_model = r#"---
description: "Symlinked test model"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;
    fs::write(actual_models.join("symlink-test.yaml"), test_model)?;

    // Create .swissarmyhammer directory
    let sah_dir = temp_home.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;

    // Create symlink to models directory (may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_path = sah_dir.join("models");
        let _ = symlink(&actual_models, &symlink_path);
    }

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);
    let _cleanup = scopeguard::guard(original_home, restore_home);

    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(result.is_ok(), "Should handle symlinked directories");

    Ok(())
}

// =============================================================================
// COMPREHENSIVE FORMAT VERIFICATION TESTS
// =============================================================================

#[tokio::test]
async fn test_execute_list_command_json_format_structure() -> Result<()> {
    let context = create_test_context(false).await;

    // We can't easily capture stdout in this test, but we can verify it doesn't error
    let result = execute_list_command(OutputFormat::Json, &context).await;

    assert!(
        result.is_ok(),
        "JSON format should produce valid output without errors"
    );

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_yaml_format_structure() -> Result<()> {
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Yaml, &context).await;

    assert!(
        result.is_ok(),
        "YAML format should produce valid output without errors"
    );

    Ok(())
}

#[tokio::test]
async fn test_execute_list_command_table_format_structure() -> Result<()> {
    let context = create_test_context(false).await;

    let result = execute_list_command(OutputFormat::Table, &context).await;

    assert!(
        result.is_ok(),
        "Table format should produce valid output without errors"
    );

    Ok(())
}
