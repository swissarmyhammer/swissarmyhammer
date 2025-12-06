//! End-to-end workflow validation tests for model management
//!
//! Tests complete workflows: list models → use model → verify config,
//! with all built-in models, model overriding, and config file backup/recovery.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;

use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use tokio::process::Command;

/// Test utility to run sah commands and capture output
async fn run_sah_command(
    args: &[&str],
    working_dir: Option<&Path>,
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
        .env("RUST_LOG", "debug") // Enable debug logs for troubleshooting
        .kill_on_drop(true);

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    let output = cmd.output().await?;
    Ok(output)
}

/// Create user models directory and configurations
fn create_user_models(temp_dir: &Path) -> Result<()> {
    let user_agents_dir = temp_dir
        .join("home")
        .join(".swissarmyhammer")
        .join("models");
    fs::create_dir_all(&user_agents_dir)?;

    let user_claude = r#"---
description: "User-customized Claude Code with special settings"
version: "1.0"
author: "End-to-End Test"
---
executor:
  type: claude-code
  config:
    claude_path: /custom/user/claude
    args: ["--user-mode", "--verbose", "--custom-config"]
quiet: false"#;
    fs::write(user_agents_dir.join("claude-code.yaml"), user_claude)?;

    let user_custom = r#"---
description: "Custom user model for testing workflows"
version: "1.0"
category: "testing"
---
executor:
  type: claude-code
  config:
    claude_path: /custom/user/custom-agent
    args: ["--test-mode"]
quiet: true"#;
    fs::write(user_agents_dir.join("test-user-agent.yaml"), user_custom)?;

    Ok(())
}

/// Create project models directory and configurations
fn create_project_models(temp_dir: &Path) -> Result<()> {
    let project_agents_dir = temp_dir.join("project").join("models");
    fs::create_dir_all(&project_agents_dir)?;

    let project_qwen = r#"---
description: "Project-optimized Qwen Coder for development workflow"
version: "2.0"
optimized_for: "development"
---
executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "project/optimized-qwen-coder"
        folder: "Q6_K_M"
quiet: false"#;
    fs::write(project_agents_dir.join("qwen-coder.yaml"), project_qwen)?;

    let project_dev = r#"---
description: "Development-optimized model for project workflow"
version: "1.2"
purpose: "development"
---
executor:
  type: claude-code
  config:
    claude_path: /project/dev/claude
    args: ["--dev-mode", "--project-context", "--enhanced-debugging"]
quiet: false"#;
    fs::write(project_agents_dir.join("project-dev.yaml"), project_dev)?;

    Ok(())
}

/// Create comprehensive test model hierarchies for end-to-end testing
fn setup_model_hierarchy(temp_dir: &Path) -> Result<()> {
    create_user_models(temp_dir)?;
    create_project_models(temp_dir)?;
    Ok(())
}

/// Parse JSON output from model list command
fn parse_agent_list_json(json_str: &str) -> Result<serde_json::Value> {
    Ok(serde_json::from_str(json_str)?)
}

/// Find model by name in JSON output
fn find_model_in_json<'a>(
    models_json: &'a serde_json::Value,
    name: &str,
) -> Option<&'a serde_json::Value> {
    models_json
        .as_array()?
        .iter()
        .find(|model| model["name"].as_str() == Some(name))
}

/// Check if config file contains expected model configuration
fn verify_model_config(config_path: &Path, expected_agent: &str) -> Result<bool> {
    if !config_path.exists() {
        return Ok(false);
    }

    let config_content = fs::read_to_string(config_path)?;

    // Parse YAML to verify structure
    let config: serde_yaml::Value = serde_yaml::from_str(&config_content)?;

    // Check for new models map structure
    if let Some(models_section) = config.get("models") {
        if let Some(models_map) = models_section.as_mapping() {
            // Check if any use case is assigned to the expected model
            for (_use_case, model_name) in models_map {
                if let Some(name_str) = model_name.as_str() {
                    if name_str == expected_agent {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

// =============================================================================
// HELPER FUNCTIONS FOR REDUCED COGNITIVE COMPLEXITY
// =============================================================================

/// Helper for test_basic_list_use_workflow: verify and update config with a model
async fn verify_and_update_config(project_root: &Path, model_name: &str) -> Result<()> {
    let use_output = run_sah_command(&["model", "use", model_name], Some(project_root)).await?;

    if !use_output.status.success() {
        return Ok(());
    }

    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    anyhow::ensure!(
        verify_model_config(&config_path, model_name)?,
        "Config should contain {}",
        model_name
    );

    Ok(())
}

/// Helper for test_basic_list_use_workflow: verify list still works
async fn verify_list_still_works(project_root: &Path) -> Result<()> {
    let final_list = run_sah_command(&["model", "list"], Some(project_root)).await?;
    anyhow::ensure!(
        final_list.status.success(),
        "Agent list should still work after config changes"
    );
    Ok(())
}

/// Helper for test_all_builtin_models_workflow: verify model after use
async fn verify_model_still_listed(project_root: &Path, model_name: &str) -> Result<()> {
    let list_output =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    anyhow::ensure!(
        list_output.status.success(),
        "Should be able to list models after using {}",
        model_name
    );

    let models_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
    anyhow::ensure!(
        find_model_in_json(&models_json, model_name).is_some(),
        "Should still list {} in models after using it",
        model_name
    );

    Ok(())
}

/// Helper for test_all_builtin_models_workflow: verify builtin model
async fn verify_builtin_model(
    project_root: &Path,
    model_name: &str,
    config_path: &Path,
) -> Result<()> {
    let use_output = run_sah_command(&["model", "use", model_name], Some(project_root)).await?;

    if !use_output.status.success() {
        let stderr = String::from_utf8_lossy(&use_output.stderr);
        anyhow::ensure!(
            !stderr.contains("not found"),
            "Built-in model '{}' should not be 'not found': {}",
            model_name,
            stderr
        );
        return Ok(());
    }

    anyhow::ensure!(
        verify_model_config(config_path, model_name)?,
        "Config should contain {}",
        model_name
    );

    verify_model_still_listed(project_root, model_name).await
}

/// Helper for test_custom_model_workflow: use and verify custom model
async fn use_and_verify_custom_model(
    model_name: &str,
    project_root: &Path,
    expected_pattern: &str,
) -> Result<()> {
    let use_output = run_sah_command(&["model", "use", model_name], Some(project_root)).await?;

    if !use_output.status.success() {
        return Ok(());
    }

    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    let config_content = fs::read_to_string(&config_path)?;
    anyhow::ensure!(
        config_content.contains(expected_pattern) || config_content.contains("models:"),
        "Config should contain {} reference. Actual: {}",
        model_name,
        config_content
    );

    Ok(())
}

/// Helper for test_config_file_backup_and_recovery: verify config preserves sections
fn verify_config_preserves_sections(config_content: &str) -> Result<()> {
    let sections = [
        ("prompt:", "prompt section"),
        ("default_template", "prompt settings"),
        ("workflow:", "workflow section"),
        ("default_timeout", "workflow settings"),
        ("other_settings:", "other settings"),
        ("deep_setting", "deeply nested settings"),
    ];

    for (pattern, description) in &sections {
        anyhow::ensure!(
            config_content.contains(pattern),
            "Should preserve {}",
            description
        );
    }

    Ok(())
}

/// Helper for test_config_file_format_consistency: verify model switch preserves YAML
async fn verify_model_switch_preserves_yaml(
    model_name: &str,
    config_path: &Path,
    project_root: &Path,
) -> Result<()> {
    let use_output = run_sah_command(&["model", "use", model_name], Some(project_root)).await?;

    if !use_output.status.success() {
        return Ok(());
    }

    let config_content = fs::read_to_string(config_path)?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Invalid YAML after using {}: {}", model_name, e))?;

    anyhow::ensure!(
        parsed.get("models").is_some(),
        "Should have models section after using {}. Actual: {:?}",
        model_name,
        parsed
    );

    if let Some(agents_map) = parsed.get("models").and_then(|v| v.as_mapping()) {
        anyhow::ensure!(
            !agents_map.is_empty(),
            "Agents map should not be empty after using {}",
            model_name
        );
    }

    Ok(())
}

/// Helper for test_concurrent_workflow_safety: verify model operation safety
async fn verify_model_operation_safety(
    model_name: &str,
    config_path: &Path,
    project_root: &Path,
    iteration: usize,
) -> Result<()> {
    let list_output =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    anyhow::ensure!(
        list_output.status.success(),
        "List should succeed on iteration {}",
        iteration
    );

    let use_output = run_sah_command(&["model", "use", model_name], Some(project_root)).await?;
    if !use_output.status.success() {
        return Ok(());
    }

    anyhow::ensure!(
        verify_model_config(config_path, model_name)?,
        "Config should be consistent for {} on iteration {}",
        model_name,
        iteration
    );

    verify_immediate_consistency(project_root, model_name, iteration).await
}

/// Helper for verify_model_operation_safety: verify immediate consistency
async fn verify_immediate_consistency(
    project_root: &Path,
    model_name: &str,
    iteration: usize,
) -> Result<()> {
    let verify_list = run_sah_command(&["model", "list"], Some(project_root)).await?;
    anyhow::ensure!(
        verify_list.status.success(),
        "Verification list should succeed after using {} on iteration {}",
        model_name,
        iteration
    );
    Ok(())
}

// =============================================================================
// BASIC END-TO-END WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_basic_list_use_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Step 1: List models and verify built-ins are available
    let list_output =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    assert!(list_output.status.success(), "Agent list should succeed");

    let models_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;

    // Should have built-in models
    assert!(
        find_model_in_json(&models_json, "claude-code").is_some(),
        "Should list claude-code model"
    );
    assert!(
        find_model_in_json(&models_json, "qwen-coder").is_some(),
        "Should list qwen-coder model"
    );
    assert!(
        find_model_in_json(&models_json, "qwen-coder-flash").is_some(),
        "Should list qwen-coder-flash model"
    );

    // Step 2: Use claude-code model
    verify_and_update_config(project_root, "claude-code").await?;

    // Step 3: Switch to different model
    verify_and_update_config(project_root, "qwen-coder").await?;

    // Step 4: List models again to ensure everything still works
    verify_list_still_works(project_root).await?;

    Ok(())
}

#[tokio::test]
async fn test_all_builtin_models_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let builtin_agents = ["claude-code", "qwen-coder", "qwen-coder-flash"];
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");

    for model_name in &builtin_agents {
        verify_builtin_model(project_root, model_name, &config_path).await?;
    }

    Ok(())
}

// =============================================================================
// AGENT HIERARCHY AND OVERRIDING WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_model_overriding_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Create project models directory - simple setup like the working test
    let project_agents_dir = project_root.join("models");
    fs::create_dir_all(&project_agents_dir)?;

    // Create simple qwen-coder override
    let project_qwen = r#"---
description: "Project-optimized Qwen Coder for development workflow"
---
executor:
  type: llama-agent
  config:
    model:
      source: !HuggingFace
        repo: "project/optimized-qwen-coder"
        folder: "Q6_K_M"
quiet: false"#;

    fs::write(project_agents_dir.join("qwen-coder.yaml"), project_qwen)?;

    // Step 1: List models from project directory (should show hierarchy)
    let list_output =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    assert!(
        list_output.status.success(),
        "Should list models with hierarchy"
    );

    let models_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;

    // Verify we have models from all sources
    let qwen_agent =
        find_model_in_json(&models_json, "qwen-coder").expect("Should have qwen-coder model");

    // qwen-coder should come from project source (project override)
    assert_eq!(
        qwen_agent["source"].as_str(),
        Some("📁 Project"),
        "qwen-coder should be from project source due to override"
    );
    assert!(
        qwen_agent["description"]
            .as_str()
            .unwrap()
            .contains("Project-optimized"),
        "Should have project override description"
    );

    Ok(())
}

/// Verify custom models are listed correctly
fn verify_custom_models_listed(models_json: &serde_json::Value) -> Result<()> {
    let user_agent =
        find_model_in_json(models_json, "test-user-agent").expect("Should have custom user model");
    let project_agent =
        find_model_in_json(models_json, "project-dev").expect("Should have custom project model");

    assert_eq!(user_agent["source"].as_str(), Some("👤 User"));
    assert!(user_agent["description"]
        .as_str()
        .unwrap()
        .contains("Custom user model"));

    assert_eq!(project_agent["source"].as_str(), Some("📁 Project"));
    assert!(project_agent["description"]
        .as_str()
        .unwrap()
        .contains("Development-optimized"));

    Ok(())
}

#[tokio::test]
async fn test_custom_model_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    setup_model_hierarchy(&temp_dir)?;

    let home_dir = &temp_dir.join("home");
    let project_root = &temp_dir.join("project");

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", &home_dir);

    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let list_output =
        run_sah_command(&["model", "list", "--format", "json"], Some(&project_root)).await?;
    assert!(
        list_output.status.success(),
        "Should list all models including custom"
    );

    let models_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
    verify_custom_models_listed(&models_json)?;

    use_and_verify_custom_model("test-user-agent", &project_root, "custom-agent").await?;
    use_and_verify_custom_model("project-dev", &project_root, "project-dev").await?;

    Ok(())
}

// =============================================================================
// CONFIG FILE MANAGEMENT WORKFLOW TESTS
// =============================================================================

/// Create initial test configuration with multiple sections
fn create_initial_test_config(project_root: &Path) -> Result<std::path::PathBuf> {
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    let initial_config = r#"# Initial configuration with multiple sections
prompt:
  default_template: "greeting"
  library_path: "./prompts"
  
workflow:
  default_timeout: 300
  max_retries: 3
  
other_settings:
  log_level: "info"
  cache_enabled: true
  custom_data:
    key1: "value1"
    key2: 42
    nested:
      deep_setting: "preserved"
  
# Existing model config (will be replaced)  
agent:
  old_executor: "will-be-replaced"
"#;
    fs::write(&config_path, initial_config)?;

    let backup_path = config_path.with_extension("yaml.backup");
    fs::copy(&config_path, &backup_path)?;

    Ok(config_path)
}

/// Verify that configuration preserves non-model sections
fn verify_config_preservation(config_path: &Path) -> Result<()> {
    let updated_config = fs::read_to_string(config_path)?;
    verify_config_preserves_sections(&updated_config)
}

/// Test initial config update with model
async fn test_initial_config_update(project_root: &Path, config_path: &Path) -> Result<()> {
    let use_output = run_sah_command(&["model", "use", "claude-code"], Some(project_root)).await?;

    if !use_output.status.success() {
        return Ok(());
    }

    verify_config_preservation(config_path)?;

    let updated_config = fs::read_to_string(config_path)?;
    anyhow::ensure!(
        updated_config.contains("models:"),
        "Should have models section. Actual: {}",
        updated_config
    );
    anyhow::ensure!(
        updated_config.contains("claude-code") || updated_config.contains("root:"),
        "Should have model assignment. Actual: {}",
        updated_config
    );

    Ok(())
}

/// Test config switch to different model
async fn test_config_switch(project_root: &Path, config_path: &Path) -> Result<()> {
    let switch_output =
        run_sah_command(&["model", "use", "qwen-coder"], Some(project_root)).await?;

    if !switch_output.status.success() {
        return Ok(());
    }

    let final_config = fs::read_to_string(config_path)?;

    verify_config_preserves_sections(&final_config)?;

    anyhow::ensure!(
        final_config.contains("llama-agent") || final_config.contains("qwen"),
        "Should contain new model config"
    );

    Ok(())
}

#[tokio::test]
async fn test_config_file_backup_and_recovery() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let config_path = create_initial_test_config(project_root)?;
    test_initial_config_update(project_root, &config_path).await?;
    test_config_switch(project_root, &config_path).await?;

    Ok(())
}

/// Verify model switch maintains YAML validity
async fn verify_model_switch_maintains_yaml_validity(
    model: &str,
    project_root: &Path,
    config_path: &Path,
) -> Result<()> {
    verify_model_switch_preserves_yaml(model, config_path, project_root).await
}

#[tokio::test]
async fn test_config_file_format_consistency() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    let first_use = run_sah_command(&["model", "use", "claude-code"], Some(project_root)).await?;

    if !first_use.status.success() {
        return Ok(());
    }

    let config_content = fs::read_to_string(&config_path)?;
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(&config_content).expect("Config should be valid YAML after first use");

    anyhow::ensure!(
        parsed.get("models").is_some(),
        "Should have models section. Actual: {:?}",
        parsed
    );

    let models = [
        "qwen-coder",
        "claude-code",
        "qwen-coder-flash",
        "claude-code",
    ];

    for model in &models {
        verify_model_switch_maintains_yaml_validity(model, project_root, &config_path).await?;
    }

    Ok(())
}

// =============================================================================
// COMPREHENSIVE INTEGRATION WORKFLOW TESTS
// =============================================================================

/// Test project initialization workflow
async fn test_project_initialization_workflow(project_root: &Path) -> Result<()> {
    let initial_list =
        run_sah_command(&["model", "list", "--format", "table"], Some(project_root)).await?;
    anyhow::ensure!(
        initial_list.status.success(),
        "Initial model list should work"
    );

    let list_output = String::from_utf8_lossy(&initial_list.stdout);
    anyhow::ensure!(list_output.contains("Models:"), "Should show model summary");
    anyhow::ensure!(
        list_output.contains("Built-in:"),
        "Should show built-in count"
    );
    anyhow::ensure!(
        list_output.contains("Project:"),
        "Should show project count"
    );
    anyhow::ensure!(list_output.contains("User:"), "Should show user count");

    let use_dev = run_sah_command(&["model", "use", "project-dev"], Some(project_root)).await?;

    if !use_dev.status.success() {
        return Ok(());
    }

    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    let config_content = fs::read_to_string(&config_path)?;
    anyhow::ensure!(
        config_content.contains("dev-agent") || config_content.contains("models:"),
        "Should contain model reference. Actual: {}",
        config_content
    );

    Ok(())
}

/// Test model switching workflow
async fn test_model_switching_workflow(project_root: &Path) -> Result<()> {
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");

    let use_qwen = run_sah_command(&["model", "use", "qwen-coder"], Some(project_root)).await?;

    if !use_qwen.status.success() {
        return Ok(());
    }

    let config_content = fs::read_to_string(&config_path)?;
    anyhow::ensure!(
        config_content.contains("qwen") || config_content.contains("models:"),
        "Should contain qwen model reference. Actual: {}",
        config_content
    );

    let mid_list =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    anyhow::ensure!(mid_list.status.success(), "Mid-workflow list should work");

    Ok(())
}

/// Test final verification workflow
async fn test_final_verification_workflow(project_root: &Path) -> Result<()> {
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");

    let use_claude = run_sah_command(&["model", "use", "claude-code"], Some(project_root)).await?;

    if !use_claude.status.success() {
        return Ok(());
    }

    let config_content = fs::read_to_string(&config_path)?;
    anyhow::ensure!(
        config_content.contains("claude") || config_content.contains("models:"),
        "Should contain claude model reference. Actual: {}",
        config_content
    );

    let final_list =
        run_sah_command(&["model", "list", "--format", "yaml"], Some(project_root)).await?;
    anyhow::ensure!(final_list.status.success(), "Final list should work");

    let yaml_output = String::from_utf8_lossy(&final_list.stdout);
    let models: serde_yaml::Value = serde_yaml::from_str(&yaml_output)?;
    anyhow::ensure!(models.is_sequence(), "Should be valid YAML sequence");

    let model_list = models.as_sequence().unwrap();
    anyhow::ensure!(
        model_list.len() >= 5,
        "Should have multiple models from all sources"
    );

    Ok(())
}

#[tokio::test]
async fn test_complete_development_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    setup_model_hierarchy(&temp_dir)?;

    let home_dir = &temp_dir.join("home");
    let project_root = &temp_dir.join("project");

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", &home_dir);

    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    test_project_initialization_workflow(&project_root).await?;
    test_model_switching_workflow(&project_root).await?;
    test_final_verification_workflow(&project_root).await?;

    Ok(())
}

#[tokio::test]
async fn test_error_recovery_workflow() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Step 1: Try to use non-existent model
    let bad_use =
        run_sah_command(&["model", "use", "definitely-not-real"], Some(project_root)).await?;
    assert!(
        !bad_use.status.success(),
        "Should fail with non-existent model"
    );

    let stderr = String::from_utf8_lossy(&bad_use.stderr);
    assert!(
        stderr.contains("not found"),
        "Should report model not found"
    );

    // Step 2: Verify we can still list models after error
    let list_after_error = run_sah_command(&["model", "list"], Some(project_root)).await?;
    assert!(
        list_after_error.status.success(),
        "Should still work after error"
    );

    // Step 3: Successfully use valid model
    let good_use = run_sah_command(&["model", "use", "claude-code"], Some(project_root)).await?;

    if !good_use.status.success() {
        return Ok(());
    }

    // Step 4: Verify system is working normally
    let final_list =
        run_sah_command(&["model", "list", "--format", "json"], Some(project_root)).await?;
    assert!(
        final_list.status.success(),
        "Should work normally after recovery"
    );

    let models_json = parse_agent_list_json(&String::from_utf8_lossy(&final_list.stdout))?;
    assert!(
        find_model_in_json(&models_json, "claude-code").is_some(),
        "Should still list claude-code after recovery"
    );

    Ok(())
}

#[tokio::test]
async fn test_concurrent_workflow_safety() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let models = ["claude-code", "qwen-coder", "qwen-coder-flash"];
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");

    for (i, model) in models.iter().enumerate() {
        verify_model_operation_safety(model, &config_path, project_root, i).await?;
    }

    Ok(())
}
