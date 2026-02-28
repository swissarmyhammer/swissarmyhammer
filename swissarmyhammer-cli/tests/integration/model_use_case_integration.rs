//! Integration tests for model selection
//!
//! Tests that verify the model system works end-to-end, including:
//! - CLI commands for showing and setting the current model
//! - Config file persistence
//! - Model resolution and defaults
//! - Global model overrides

use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissarmyhammerDirectory;
use tokio::process::Command;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Get the path to the sah binary
fn get_sah_binary_path() -> String {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sah") {
        path
    } else {
        format!(
            "{}/target/debug/sah",
            env!("CARGO_MANIFEST_DIR").replace("/swissarmyhammer-cli", "")
        )
    }
}

/// Test utility to run sah model commands
async fn run_model_command(args: &[&str]) -> Result<std::process::Output> {
    let binary_path = get_sah_binary_path();

    let mut cmd = Command::new(&binary_path);
    cmd.args(args).env("RUST_LOG", "error").kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

/// Test utility to run sah model commands in a specific directory
async fn run_model_command_in_dir(
    args: &[&str],
    working_dir: &Path,
) -> Result<std::process::Output> {
    let binary_path = get_sah_binary_path();

    let mut cmd = Command::new(&binary_path);
    cmd.args(args)
        .current_dir(working_dir)
        .env("RUST_LOG", "error")
        .kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

/// Assert that stdout contains the given model name
fn assert_stdout_contains_model(stdout: &str, model_name: &str) {
    assert!(
        stdout.contains(model_name),
        "Should show {} model",
        model_name
    );
}

/// Assert that config file contains the expected model setting
fn assert_config_contains_model(
    project_root: &Path,
    expected_model: &str,
) -> Result<String> {
    let config_path = project_root
        .join(SwissarmyhammerDirectory::dir_name())
        .join("sah.yaml");
    assert!(config_path.exists(), "Config file should be created");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("model:"), "Should have model key");
    assert!(
        config.contains(expected_model),
        "Should contain model name: {}",
        expected_model
    );

    Ok(config)
}

/// Create a test config file with the given content
fn create_test_config(project_root: &Path, content: &str) -> Result<PathBuf> {
    let sah_dir = project_root.join(SwissarmyhammerDirectory::dir_name());
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    fs::write(&config_path, content)?;
    Ok(config_path)
}

/// Test context for managing temporary directories and running commands
struct TestContext {
    _env: IsolatedTestEnvironment,
    project_root: PathBuf,
}

impl TestContext {
    fn new() -> Result<Self> {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let project_root = env.temp_dir();
        Ok(Self {
            _env: env,
            project_root,
        })
    }

    async fn run_command(&self, args: &[&str]) -> Result<std::process::Output> {
        run_model_command_in_dir(args, &self.project_root).await
    }

    fn create_config(&self, content: &str) -> Result<PathBuf> {
        create_test_config(&self.project_root, content)
    }

    fn assert_config_contains_model(&self, expected_model: &str) -> Result<String> {
        assert_config_contains_model(&self.project_root, expected_model)
    }
}

// =============================================================================
// MODEL SHOW COMMAND TESTS
// =============================================================================

#[tokio::test]
async fn test_model_show_command_no_config() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx.run_command(&["model", "show"]).await?;

    assert!(output.status.success(), "model show should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Show should display model information
    assert!(
        stdout.contains("Model") || stdout.contains("model"),
        "Should show model information"
    );

    Ok(())
}

#[tokio::test]
async fn test_model_show_command_with_config() -> Result<()> {
    let ctx = TestContext::new()?;

    let config_content = "model: qwen-coder\n";
    ctx.create_config(config_content)?;

    let output = ctx.run_command(&["model", "show"]).await?;

    assert!(
        output.status.success(),
        "model show should succeed with config"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_stdout_contains_model(&stdout, "qwen-coder");

    Ok(())
}

// =============================================================================
// MODEL USE COMMAND TESTS
// =============================================================================

#[tokio::test]
async fn test_model_use_sets_model() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Command failed with stderr: {}", stderr);
    }

    assert!(
        output.status.success(),
        "model use qwen-coder should succeed"
    );

    ctx.assert_config_contains_model("qwen-coder")?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_claude_code() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx.run_command(&["model", "use", "claude-code"]).await?;

    assert!(
        output.status.success(),
        "model use claude-code should succeed"
    );

    ctx.assert_config_contains_model("claude-code")?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_replaces_previous() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "claude-code"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    ctx.assert_config_contains_model("qwen-coder")?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_nonexistent_model() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "nonexistent-agent"])
        .await?;

    assert!(
        !output.status.success(),
        "Should fail for nonexistent model"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "Should report model not found"
    );
    assert!(
        stderr.contains("nonexistent-agent"),
        "Should mention the model name"
    );

    Ok(())
}

// =============================================================================
// GLOBAL MODEL OVERRIDE TESTS
// =============================================================================

#[tokio::test]
async fn test_global_model_override_flag() -> Result<()> {
    let ctx = TestContext::new()?;

    let config_content = "model: qwen-coder\n";
    ctx.create_config(config_content)?;

    let output = ctx
        .run_command(&["--model", "claude-code", "model", "show"])
        .await?;

    assert!(
        output.status.success(),
        "Global --model flag should be accepted"
    );

    Ok(())
}

// =============================================================================
// CONFIG PRESERVATION TESTS
// =============================================================================

#[tokio::test]
async fn test_model_use_preserves_other_config() -> Result<()> {
    let ctx = TestContext::new()?;

    let existing_config = r#"# Existing configuration
other_section:
  value: "preserved"
  number: 42
model: claude-code
"#;
    ctx.create_config(existing_config)?;

    let output = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    assert!(
        output.status.success(),
        "model use should update existing config"
    );

    let config = ctx.assert_config_contains_model("qwen-coder")?;
    assert!(
        config.contains("other_section:"),
        "Should preserve other_section"
    );
    assert!(
        config.contains("value: preserved"),
        "Should preserve existing values"
    );
    assert!(
        config.contains("number: 42"),
        "Should preserve number field"
    );

    Ok(())
}

#[tokio::test]
async fn test_model_use_updates_existing_model() -> Result<()> {
    let ctx = TestContext::new()?;

    let initial_config = "model: qwen-coder\n";
    ctx.create_config(initial_config)?;

    let output = ctx
        .run_command(&["model", "use", "claude-code"])
        .await?;

    assert!(
        output.status.success(),
        "model use should update existing model"
    );

    let updated_config = ctx.assert_config_contains_model("claude-code")?;
    assert!(
        !updated_config.contains("qwen-coder"),
        "Should not have qwen-coder anymore"
    );

    Ok(())
}

// =============================================================================
// HELP AND USAGE TESTS
// =============================================================================

#[tokio::test]
async fn test_model_show_help() -> Result<()> {
    let output = run_model_command(&["model", "show", "--help"]).await?;

    assert!(output.status.success(), "model show --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("show") || stdout.contains("Show"),
        "Help should mention show command"
    );

    Ok(())
}

#[tokio::test]
async fn test_model_use_help_shows_name_parameter() -> Result<()> {
    let output = run_model_command(&["model", "use", "--help"]).await?;

    assert!(output.status.success(), "model use --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("NAME") || stdout.contains("name"),
        "Help should show NAME positional argument"
    );
    assert!(
        stdout.contains("Model name") || stdout.contains("model name") || stdout.contains("MODEL"),
        "Help should mention model name"
    );

    Ok(())
}

// =============================================================================
// END-TO-END WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_workflow_initial_state_verification() -> Result<()> {
    let ctx = TestContext::new()?;

    let show_output = ctx.run_command(&["model", "show"]).await?;
    assert!(
        show_output.status.success(),
        "Initial model show should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_workflow_sequential_model_configuration() -> Result<()> {
    let ctx = TestContext::new()?;

    let use_output = ctx
        .run_command(&["model", "use", "claude-code"])
        .await?;
    assert!(
        use_output.status.success(),
        "Setting model should succeed"
    );

    let switch_output = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;
    assert!(
        switch_output.status.success(),
        "Switching model should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_workflow_final_state_verification() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    let final_show_output = ctx.run_command(&["model", "show"]).await?;
    assert!(
        final_show_output.status.success(),
        "Final model show should succeed"
    );

    let final_stdout = String::from_utf8_lossy(&final_show_output.stdout);
    assert_stdout_contains_model(&final_stdout, "qwen-coder");

    Ok(())
}

#[tokio::test]
async fn test_workflow_config_file_validation() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    ctx.assert_config_contains_model("qwen-coder")?;

    Ok(())
}

#[tokio::test]
async fn test_model_configuration_persistence() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "qwen-coder"])
        .await?;

    let config_path = ctx
        .project_root
        .join(SwissarmyhammerDirectory::dir_name())
        .join("sah.yaml");
    let config_content = fs::read_to_string(config_path)?;

    let config_value: serde_yaml::Value = serde_yaml::from_str(&config_content)?;

    assert!(
        config_value.get("model").is_some(),
        "Config should have model key"
    );

    let model_value = config_value
        .get("model")
        .expect("model should exist")
        .as_str()
        .expect("model should be a string");

    assert_eq!(
        model_value, "qwen-coder",
        "Should have model: qwen-coder"
    );

    Ok(())
}
