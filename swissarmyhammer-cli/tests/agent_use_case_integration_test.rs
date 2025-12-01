//! Integration tests for use case-based agent selection
//!
//! Tests that verify the agent use case system works end-to-end, including:
//! - CLI commands for showing and setting use case agents
//! - Config file persistence
//! - Use case resolution and fallback
//! - Global agent overrides

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio::process::Command;

/// Test utility to run sah agent commands
async fn run_agent_command(args: &[&str]) -> Result<std::process::Output> {
    let binary_path = if let Ok(path) = std::env::var("CARGO_BIN_EXE_sah") {
        path
    } else {
        format!(
            "{}/target/debug/sah",
            env!("CARGO_MANIFEST_DIR").replace("/swissarmyhammer-cli", "")
        )
    };

    let mut cmd = Command::new(&binary_path);
    cmd.args(args).env("RUST_LOG", "error").kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

/// Test utility to run sah agent commands in a specific directory
async fn run_agent_command_in_dir(
    args: &[&str],
    working_dir: &Path,
) -> Result<std::process::Output> {
    let binary_path = if let Ok(path) = std::env::var("CARGO_BIN_EXE_sah") {
        path
    } else {
        format!(
            "{}/target/debug/sah",
            env!("CARGO_MANIFEST_DIR").replace("/swissarmyhammer-cli", "")
        )
    };

    let mut cmd = Command::new(&binary_path);
    cmd.args(args)
        .current_dir(working_dir)
        .env("RUST_LOG", "error")
        .kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

// =============================================================================
// AGENT SHOW COMMAND TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_show_command_no_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    let output = run_agent_command_in_dir(&["agent", "show"], project_root).await?;

    assert!(output.status.success(), "agent show should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show default configuration
    assert!(
        stdout.contains("root") || stdout.contains("Root"),
        "Should show root use case"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_show_command_with_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create config with use case agents
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    let config_content = r#"agents:
  root: "claude-code"
  rules: "qwen-coder"
  workflows: "claude-code"
"#;
    fs::write(&config_path, config_content)?;

    let output = run_agent_command_in_dir(&["agent", "show"], project_root).await?;

    assert!(
        output.status.success(),
        "agent show should succeed with config"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show all use cases
    assert!(
        stdout.contains("root") || stdout.contains("Root"),
        "Should show root use case"
    );
    assert!(
        stdout.contains("rules") || stdout.contains("Rules"),
        "Should show rules use case"
    );
    assert!(
        stdout.contains("workflows") || stdout.contains("Workflows"),
        "Should show workflows use case"
    );

    // Should show agent names
    assert!(
        stdout.contains("claude-code"),
        "Should show claude-code agent"
    );
    assert!(
        stdout.contains("qwen-coder"),
        "Should show qwen-coder agent"
    );

    Ok(())
}

// =============================================================================
// AGENT USE WITH USE CASE TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_use_with_use_case_rules() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Set Rules use case
    let output =
        run_agent_command_in_dir(&["agent", "use", "rules", "qwen-coder"], project_root).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Command failed with stderr: {}", stderr);
    }

    assert!(
        output.status.success(),
        "agent use rules qwen-coder should succeed"
    );

    // Verify config was written
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    assert!(config_path.exists(), "Config file should be created");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("agents:"), "Should have agents section");
    assert!(
        config.contains("rules:") && config.contains("qwen-coder"),
        "Should have rules: qwen-coder"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_with_use_case_workflows() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Set Workflows use case
    let output =
        run_agent_command_in_dir(&["agent", "use", "workflows", "claude-code"], project_root)
            .await?;

    assert!(
        output.status.success(),
        "agent use workflows claude-code should succeed"
    );

    // Verify config was written
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    assert!(config_path.exists(), "Config file should be created");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("agents:"), "Should have agents section");
    assert!(
        config.contains("workflows:") && config.contains("claude-code"),
        "Should have workflows: claude-code"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_root_backward_compatibility() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Use agent without specifying use case (should set root)
    let output = run_agent_command_in_dir(&["agent", "use", "claude-code"], project_root).await?;

    assert!(
        output.status.success(),
        "agent use claude-code should succeed"
    );

    // Verify config was written with root
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    assert!(config_path.exists(), "Config file should be created");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("agents:"), "Should have agents section");
    assert!(
        config.contains("root:") && config.contains("claude-code"),
        "Should have root: claude-code"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_multiple_use_cases() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Set multiple use cases
    let _ =
        run_agent_command_in_dir(&["agent", "use", "root", "claude-code"], project_root).await?;
    let _ =
        run_agent_command_in_dir(&["agent", "use", "rules", "qwen-coder"], project_root).await?;
    let _ = run_agent_command_in_dir(&["agent", "use", "workflows", "claude-code"], project_root)
        .await?;

    // Verify all use cases are in config
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    let config = fs::read_to_string(config_path)?;

    assert!(
        config.contains("root:") && config.contains("claude-code"),
        "Should have root: claude-code"
    );
    assert!(
        config.contains("rules:") && config.contains("qwen-coder"),
        "Should have rules: qwen-coder"
    );
    assert!(
        config.contains("workflows:") && config.contains("claude-code"),
        "Should have workflows: claude-code"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_invalid_use_case() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Try to use invalid use case
    let output = run_agent_command_in_dir(
        &["agent", "use", "invalid-use-case", "claude-code"],
        project_root,
    )
    .await?;

    // Should fail with helpful error message
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
async fn test_agent_use_nonexistent_agent_for_use_case() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Try to use nonexistent agent for rules
    let output = run_agent_command_in_dir(
        &["agent", "use", "rules", "nonexistent-agent"],
        project_root,
    )
    .await?;

    assert!(
        !output.status.success(),
        "Should fail for nonexistent agent"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "Should report agent not found"
    );
    assert!(
        stderr.contains("nonexistent-agent"),
        "Should mention the agent name"
    );

    Ok(())
}

// =============================================================================
// GLOBAL AGENT OVERRIDE TESTS
// =============================================================================

#[tokio::test]
async fn test_global_agent_override_flag() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create config with different agent for rules
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    let config_content = r#"agents:
  root: "claude-code"
  rules: "qwen-coder"
"#;
    fs::write(&config_path, config_content)?;

    // The --agent flag should override config
    // We can't easily test rules check here without full setup,
    // but we can verify the flag is accepted
    let output =
        run_agent_command_in_dir(&["--agent", "claude-code", "agent", "show"], project_root)
            .await?;

    // Command should succeed with global override
    assert!(
        output.status.success(),
        "Global --agent flag should be accepted"
    );

    Ok(())
}

// =============================================================================
// CONFIG PRESERVATION TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_use_preserves_other_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create config with other sections
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    let existing_config = r#"# Existing configuration
other_section:
  value: "preserved"
  number: 42
agents:
  root: "claude-code"
"#;
    fs::write(&config_path, existing_config)?;

    // Set a use case agent
    let output =
        run_agent_command_in_dir(&["agent", "use", "rules", "qwen-coder"], project_root).await?;

    assert!(
        output.status.success(),
        "agent use should update existing config"
    );

    // Verify other sections are preserved
    let updated_config = fs::read_to_string(&config_path)?;
    assert!(
        updated_config.contains("other_section:"),
        "Should preserve other_section"
    );
    assert!(
        updated_config.contains("value: preserved"),
        "Should preserve existing values"
    );
    assert!(
        updated_config.contains("number: 42"),
        "Should preserve number field"
    );
    assert!(
        updated_config.contains("rules:") && updated_config.contains("qwen-coder"),
        "Should add rules agent"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_updates_existing_use_case() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create config with rules agent
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    let initial_config = r#"agents:
  root: "claude-code"
  rules: "qwen-coder"
"#;
    fs::write(&config_path, initial_config)?;

    // Update rules to different agent
    let output =
        run_agent_command_in_dir(&["agent", "use", "rules", "claude-code"], project_root).await?;

    assert!(
        output.status.success(),
        "agent use should update existing use case"
    );

    // Verify rules was updated
    let updated_config = fs::read_to_string(&config_path)?;
    assert!(
        updated_config.contains("rules:") && updated_config.contains("claude-code"),
        "Should update rules to claude-code"
    );
    assert!(
        !updated_config.contains("qwen-coder")
            || updated_config.matches("qwen-coder").count() == 0
            || (updated_config.contains("qwen-coder")
                && !updated_config.contains("rules: \"qwen-coder\"")),
        "Should not have qwen-coder for rules anymore"
    );

    Ok(())
}

// =============================================================================
// HELP AND USAGE TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_show_help() -> Result<()> {
    let output = run_agent_command(&["agent", "show", "--help"]).await?;

    assert!(output.status.success(), "agent show --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("show") || stdout.contains("Show"),
        "Help should mention show command"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_help_shows_use_case_parameter() -> Result<()> {
    let output = run_agent_command(&["agent", "use", "--help"]).await?;

    assert!(output.status.success(), "agent use --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check for the positional arguments FIRST and SECOND
    assert!(
        stdout.contains("FIRST") || stdout.contains("first"),
        "Help should show FIRST positional argument"
    );
    assert!(
        stdout.contains("SECOND") || stdout.contains("second"),
        "Help should show SECOND positional argument"
    );
    // Check that the help explains use case and agent name
    assert!(
        stdout.contains("use case") || stdout.contains("USE_CASE"),
        "Help should mention use case"
    );
    assert!(
        stdout.contains("Agent name") || stdout.contains("agent name") || stdout.contains("AGENT"),
        "Help should mention agent name"
    );

    Ok(())
}

// =============================================================================
// END-TO-END WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_complete_use_case_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Step 1: Show initial state (no config)
    let show_output = run_agent_command_in_dir(&["agent", "show"], project_root).await?;
    assert!(
        show_output.status.success(),
        "Initial agent show should succeed"
    );

    // Step 2: Set root agent
    let use_root_output =
        run_agent_command_in_dir(&["agent", "use", "root", "claude-code"], project_root).await?;
    assert!(
        use_root_output.status.success(),
        "Setting root agent should succeed"
    );

    // Step 3: Set rules agent
    let use_rules_output =
        run_agent_command_in_dir(&["agent", "use", "rules", "qwen-coder"], project_root).await?;
    assert!(
        use_rules_output.status.success(),
        "Setting rules agent should succeed"
    );

    // Step 4: Set workflows agent
    let use_workflows_output =
        run_agent_command_in_dir(&["agent", "use", "workflows", "claude-code"], project_root)
            .await?;
    assert!(
        use_workflows_output.status.success(),
        "Setting workflows agent should succeed"
    );

    // Step 5: Show final state
    let final_show_output = run_agent_command_in_dir(&["agent", "show"], project_root).await?;
    assert!(
        final_show_output.status.success(),
        "Final agent show should succeed"
    );

    let final_stdout = String::from_utf8_lossy(&final_show_output.stdout);
    assert!(
        final_stdout.contains("root") && final_stdout.contains("claude-code"),
        "Should show root: claude-code"
    );
    assert!(
        final_stdout.contains("rules") && final_stdout.contains("qwen-coder"),
        "Should show rules: qwen-coder"
    );
    assert!(
        final_stdout.contains("workflows") && final_stdout.contains("claude-code"),
        "Should show workflows: claude-code"
    );

    // Step 6: Verify config file content
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    assert!(config_path.exists(), "Config file should exist");

    let config = fs::read_to_string(config_path)?;
    assert!(config.contains("agents:"), "Should have agents section");
    assert!(
        config.contains("root: ") || config.contains("root:"),
        "Should have root use case"
    );
    assert!(
        config.contains("rules: ") || config.contains("rules:"),
        "Should have rules use case"
    );
    assert!(
        config.contains("workflows: ") || config.contains("workflows:"),
        "Should have workflows use case"
    );

    Ok(())
}

#[tokio::test]
async fn test_use_case_configuration_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Configure use cases
    let _ =
        run_agent_command_in_dir(&["agent", "use", "root", "claude-code"], project_root).await?;
    let _ =
        run_agent_command_in_dir(&["agent", "use", "rules", "qwen-coder"], project_root).await?;

    // Read config directly to verify structure
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    let config_content = fs::read_to_string(config_path)?;

    // Parse as YAML to verify structure
    let config_value: serde_yaml::Value = serde_yaml::from_str(&config_content)?;

    // Verify agents map exists
    assert!(
        config_value.get("agents").is_some(),
        "Config should have agents map"
    );

    let agents = config_value
        .get("agents")
        .expect("agents should exist")
        .as_mapping()
        .expect("agents should be a mapping");

    // Verify use case assignments
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
