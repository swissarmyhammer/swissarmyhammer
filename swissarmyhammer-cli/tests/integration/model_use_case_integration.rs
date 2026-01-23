//! Integration tests for use case-based model selection
//!
//! Tests that verify the model use case system works end-to-end, including:
//! - CLI commands for showing and setting use case models
//! - Config file persistence
//! - Use case resolution and fallback
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

/// Assert that stdout contains the given use case (case-insensitive check)
fn assert_stdout_contains_use_case(stdout: &str, use_case: &str) {
    assert!(
        stdout.contains(use_case) || stdout.contains(&use_case.to_ascii_lowercase()),
        "Should show {} use case",
        use_case
    );
}

/// Assert that stdout contains the given model name
fn assert_stdout_contains_model(stdout: &str, model_name: &str) {
    assert!(
        stdout.contains(model_name),
        "Should show {} model",
        model_name
    );
}

/// Assert that config file contains the expected patterns
fn assert_config_contains(
    project_root: &Path,
    expected_patterns: &[(&str, &str)],
) -> Result<String> {
    let config_path = project_root.join(SwissarmyhammerDirectory::dir_name()).join("sah.yaml");
    assert!(config_path.exists(), "Config file should be created");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("agents:"), "Should have agents section");

    for (key, value) in expected_patterns {
        assert!(
            config.contains(key) && config.contains(value),
            "Should have {}: {}",
            key,
            value
        );
    }

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

    fn assert_config_contains(&self, expected_patterns: &[(&str, &str)]) -> Result<String> {
        assert_config_contains(&self.project_root, expected_patterns)
    }
}

// =============================================================================
// AGENT SHOW COMMAND TESTS
// =============================================================================

#[tokio::test]
async fn test_model_show_command_no_config() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx.run_command(&["model", "show"]).await?;

    assert!(output.status.success(), "agent show should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_stdout_contains_use_case(&stdout, "root");

    Ok(())
}

#[tokio::test]
async fn test_model_show_command_with_config() -> Result<()> {
    let ctx = TestContext::new()?;

    let config_content = r#"agents:
  root: "claude-code"
  rules: "qwen-coder"
  workflows: "claude-code"
"#;
    ctx.create_config(config_content)?;

    let output = ctx.run_command(&["model", "show"]).await?;

    assert!(
        output.status.success(),
        "agent show should succeed with config"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_stdout_contains_use_case(&stdout, "root");
    assert_stdout_contains_use_case(&stdout, "rules");
    assert_stdout_contains_use_case(&stdout, "workflows");
    assert_stdout_contains_model(&stdout, "claude-code");
    assert_stdout_contains_model(&stdout, "qwen-coder");

    Ok(())
}

// =============================================================================
// AGENT USE WITH USE CASE TESTS
// =============================================================================

#[tokio::test]
async fn test_model_use_with_use_case_rules() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Command failed with stderr: {}", stderr);
    }

    assert!(
        output.status.success(),
        "agent use rules qwen-coder should succeed"
    );

    ctx.assert_config_contains(&[("rules:", "qwen-coder")])?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_with_use_case_workflows() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "workflows", "claude-code"])
        .await?;

    assert!(
        output.status.success(),
        "agent use workflows claude-code should succeed"
    );

    ctx.assert_config_contains(&[("workflows:", "claude-code")])?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_root_backward_compatibility() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx.run_command(&["model", "use", "claude-code"]).await?;

    assert!(
        output.status.success(),
        "agent use claude-code should succeed"
    );

    ctx.assert_config_contains(&[("root:", "claude-code")])?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_multiple_use_cases() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "root", "claude-code"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "workflows", "claude-code"])
        .await?;

    ctx.assert_config_contains(&[
        ("root:", "claude-code"),
        ("rules:", "qwen-coder"),
        ("workflows:", "claude-code"),
    ])?;

    Ok(())
}

#[tokio::test]
async fn test_model_use_invalid_use_case() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "invalid-use-case", "claude-code"])
        .await?;

    assert!(
        !output.status.success(),
        "agent use invalid-use-case should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid") || stderr.contains("Invalid"),
        "Should mention invalid use case"
    );

    Ok(())
}

#[tokio::test]
async fn test_model_use_nonexistent_model_for_use_case() -> Result<()> {
    let ctx = TestContext::new()?;

    let output = ctx
        .run_command(&["model", "use", "rules", "nonexistent-agent"])
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
// GLOBAL AGENT OVERRIDE TESTS
// =============================================================================

#[tokio::test]
async fn test_global_model_override_flag() -> Result<()> {
    let ctx = TestContext::new()?;

    let config_content = r#"models:
  root: "claude-code"
  rules: "qwen-coder"
"#;
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
models:
  root: "claude-code"
"#;
    ctx.create_config(existing_config)?;

    let output = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;

    assert!(
        output.status.success(),
        "agent use should update existing config"
    );

    let config = ctx.assert_config_contains(&[("rules:", "qwen-coder")])?;
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
async fn test_model_use_updates_existing_use_case() -> Result<()> {
    let ctx = TestContext::new()?;

    let initial_config = r#"models:
  root: "claude-code"
  rules: "qwen-coder"
"#;
    ctx.create_config(initial_config)?;

    let output = ctx
        .run_command(&["model", "use", "rules", "claude-code"])
        .await?;

    assert!(
        output.status.success(),
        "agent use should update existing use case"
    );

    let updated_config = ctx.assert_config_contains(&[("rules:", "claude-code")])?;
    assert!(
        !updated_config.contains("rules: \"qwen-coder\""),
        "Should not have qwen-coder for rules anymore"
    );

    Ok(())
}

// =============================================================================
// HELP AND USAGE TESTS
// =============================================================================

#[tokio::test]
async fn test_model_show_help() -> Result<()> {
    let output = run_model_command(&["model", "show", "--help"]).await?;

    assert!(output.status.success(), "agent show --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("show") || stdout.contains("Show"),
        "Help should mention show command"
    );

    Ok(())
}

#[tokio::test]
async fn test_model_use_help_shows_use_case_parameter() -> Result<()> {
    let output = run_model_command(&["model", "use", "--help"]).await?;

    assert!(output.status.success(), "agent use --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("FIRST") || stdout.contains("first"),
        "Help should show FIRST positional argument"
    );
    assert!(
        stdout.contains("SECOND") || stdout.contains("second"),
        "Help should show SECOND positional argument"
    );
    assert!(
        stdout.contains("use case") || stdout.contains("USE_CASE"),
        "Help should mention use case"
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
async fn test_workflow_sequential_use_case_configuration() -> Result<()> {
    let ctx = TestContext::new()?;

    let use_root_output = ctx
        .run_command(&["model", "use", "root", "claude-code"])
        .await?;
    assert!(
        use_root_output.status.success(),
        "Setting root model should succeed"
    );

    let use_rules_output = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;
    assert!(
        use_rules_output.status.success(),
        "Setting rules model should succeed"
    );

    let use_workflows_output = ctx
        .run_command(&["model", "use", "workflows", "claude-code"])
        .await?;
    assert!(
        use_workflows_output.status.success(),
        "Setting workflows model should succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_workflow_final_state_verification() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "root", "claude-code"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "workflows", "claude-code"])
        .await?;

    let final_show_output = ctx.run_command(&["model", "show"]).await?;
    assert!(
        final_show_output.status.success(),
        "Final model show should succeed"
    );

    let final_stdout = String::from_utf8_lossy(&final_show_output.stdout);
    assert_stdout_contains_use_case(&final_stdout, "root");
    assert_stdout_contains_model(&final_stdout, "claude-code");
    assert_stdout_contains_use_case(&final_stdout, "rules");
    assert_stdout_contains_model(&final_stdout, "qwen-coder");
    assert_stdout_contains_use_case(&final_stdout, "workflows");

    Ok(())
}

#[tokio::test]
async fn test_workflow_config_file_validation() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "root", "claude-code"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "workflows", "claude-code"])
        .await?;

    let config =
        ctx.assert_config_contains(&[("root:", ""), ("rules:", ""), ("workflows:", "")])?;

    assert!(config.contains("root:"), "Should have root use case");
    assert!(config.contains("rules:"), "Should have rules use case");
    assert!(
        config.contains("workflows:"),
        "Should have workflows use case"
    );

    Ok(())
}

#[tokio::test]
async fn test_use_case_configuration_persistence() -> Result<()> {
    let ctx = TestContext::new()?;

    let _ = ctx
        .run_command(&["model", "use", "root", "claude-code"])
        .await?;
    let _ = ctx
        .run_command(&["model", "use", "rules", "qwen-coder"])
        .await?;

    let config_path = ctx.project_root.join(SwissarmyhammerDirectory::dir_name()).join("sah.yaml");
    let config_content = fs::read_to_string(config_path)?;

    let config_value: serde_yaml::Value = serde_yaml::from_str(&config_content)?;

    assert!(
        config_value.get("agents").is_some(),
        "Config should have agents map"
    );

    let agents = config_value
        .get("agents")
        .expect("agents should exist")
        .as_mapping()
        .expect("agents should be a mapping");

    assert!(
        agents.get(serde_yaml::Value::String("root".to_string()))
            == Some(&serde_yaml::Value::String("claude-code".to_string())),
        "Should have root: claude-code"
    );
    assert!(
        agents.get(serde_yaml::Value::String("rules".to_string()))
            == Some(&serde_yaml::Value::String("qwen-coder".to_string())),
        "Should have rules: qwen-coder"
    );

    Ok(())
}
