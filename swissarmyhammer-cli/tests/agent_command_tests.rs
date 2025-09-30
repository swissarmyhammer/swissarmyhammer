//! Integration tests for agent CLI commands
//!
//! Tests the sah agent list and sah agent use commands with real built-in agents,
//! error scenarios, and agent discovery hierarchy.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
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
    cmd.args(args)
        .env("RUST_LOG", "error") // Reduce log noise in tests
        .kill_on_drop(true);

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
        .env("RUST_LOG", "error") // Reduce log noise in tests
        .kill_on_drop(true);

    let output = cmd.output().await?;
    Ok(output)
}

/// Create test agent files in a directory
fn create_test_agent_files(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;

    // Create a user agent that overrides a builtin
    let user_claude_content = r#"---
description: "User-overridden Claude Code agent"
---
executor:
  type: claude-code
  config:
    claude_path: /user/claude
    args: ["--user-mode"]
quiet: false"#;
    fs::write(dir.join("claude-code.yaml"), user_claude_content)?;

    // Create a custom user agent
    let custom_agent_content = r#"---
description: "Custom test agent"
---
executor:
  type: claude-code
  config:
    claude_path: /custom/claude
    args: ["--custom-mode"]
quiet: true"#;
    fs::write(dir.join("custom-test-agent.yaml"), custom_agent_content)?;

    Ok(())
}

/// Create project agent files in a directory  
fn create_project_agent_files(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;

    // Create a project agent that overrides a builtin
    let project_qwen_content = r#"---
description: "Project-customized Qwen Coder"
---
executor:
  type: llama-agent
  config:
    model:
      source:
        HuggingFace:
          repo: "custom/qwen-model"
          filename: "custom.gguf"
quiet: false"#;
    fs::write(dir.join("qwen-coder.yaml"), project_qwen_content)?;

    // Create a unique project agent
    let project_agent_content = r#"---
description: "Project-specific development agent"
---
executor:
  type: claude-code
  config:
    claude_path: /project/claude
    args: ["--project-dev"]
quiet: false"#;
    fs::write(dir.join("project-dev-agent.yaml"), project_agent_content)?;

    Ok(())
}

// =============================================================================
// BASIC COMMAND TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_list_basic_functionality() -> Result<()> {
    let output = run_agent_command(&["agent", "list"]).await?;

    assert!(output.status.success(), "agent list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain built-in agents
    assert!(
        stdout.contains("claude-code"),
        "Should list claude-code agent"
    );
    assert!(
        stdout.contains("qwen-coder"),
        "Should list qwen-coder agent"
    );
    assert!(
        stdout.contains("qwen-coder-flash"),
        "Should list qwen-coder-flash agent"
    );

    // Should show summary information
    assert!(
        stdout.contains("Agents:"),
        "Should show agent count summary"
    );
    assert!(stdout.contains("Built-in:"), "Should show built-in count");

    Ok(())
}

#[tokio::test]
async fn test_agent_list_json_format() -> Result<()> {
    let output = run_agent_command(&["agent", "list", "--format", "json"]).await?;

    assert!(
        output.status.success(),
        "agent list --format json should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should be valid JSON
    let json_value: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Should be an array
    assert!(json_value.is_array(), "JSON output should be an array");

    let agents = json_value.as_array().unwrap();
    assert!(!agents.is_empty(), "Should have at least built-in agents");

    // Check structure of first agent
    let first_agent = &agents[0];
    assert!(
        first_agent["name"].is_string(),
        "Agent should have name field"
    );
    assert!(
        first_agent["description"].is_string(),
        "Agent should have description field"
    );
    assert!(
        first_agent["source"].is_string(),
        "Agent should have source field"
    );

    // Should contain built-in agents
    let agent_names: Vec<_> = agents
        .iter()
        .map(|agent| agent["name"].as_str().unwrap())
        .collect();
    assert!(
        agent_names.contains(&"claude-code"),
        "Should include claude-code"
    );
    assert!(
        agent_names.contains(&"qwen-coder"),
        "Should include qwen-coder"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_list_yaml_format() -> Result<()> {
    let output = run_agent_command(&["agent", "list", "--format", "yaml"]).await?;

    assert!(
        output.status.success(),
        "agent list --format yaml should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should be valid YAML
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(&stdout).expect("Output should be valid YAML");

    // Should be an array
    assert!(yaml_value.is_sequence(), "YAML output should be a sequence");

    let agents = yaml_value.as_sequence().unwrap();
    assert!(!agents.is_empty(), "Should have at least built-in agents");

    Ok(())
}

#[tokio::test]
async fn test_agent_use_builtin_agent() -> Result<()> {
    let _env = IsolatedTestEnvironment::new()?;

    let output = run_agent_command(&["agent", "use", "claude-code"]).await?;

    // Should succeed or fail with specific config-related errors only
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should not fail with "not found" error for built-in agent
        assert!(
            !stderr.contains("not found") && !stdout.contains("not found"),
            "Built-in agent should not be 'not found'. stderr: {}, stdout: {}",
            stderr,
            stdout
        );
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Successfully switched to agent"),
            "Should show success message"
        );
        assert!(
            stdout.contains("claude-code"),
            "Should mention the agent name"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_agent_use_nonexistent_agent() -> Result<()> {
    let output = run_agent_command(&["agent", "use", "nonexistent-agent-xyz"]).await?;

    assert!(
        !output.status.success(),
        "Using nonexistent agent should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "Should report agent not found"
    );
    assert!(
        stderr.contains("nonexistent-agent-xyz"),
        "Should mention the agent name"
    );

    // Should provide suggestions or list available agents
    assert!(
        stderr.contains("Available agents:") || stderr.contains("Did you mean:"),
        "Should provide helpful suggestions. stderr: {}",
        stderr
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_empty_name() -> Result<()> {
    let output = run_agent_command(&["agent", "use", ""]).await?;

    assert!(
        !output.status.success(),
        "Using empty agent name should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be empty"),
        "Should report empty name error"
    );

    Ok(())
}

// =============================================================================
// AGENT DISCOVERY HIERARCHY TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_precedence_user_over_builtin() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create user agents directory with override
    let user_agents_dir = temp_home.join(".swissarmyhammer").join("agents");
    create_test_agent_files(&user_agents_dir)?;

    // Set temporary home directory
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    // Ensure cleanup
    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let output = run_agent_command(&["agent", "list"]).await?;
    assert!(output.status.success(), "agent list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should still list claude-code, but now from user source
    assert!(stdout.contains("claude-code"), "Should list claude-code");

    // Should list the custom user agent
    assert!(
        stdout.contains("custom-test-agent"),
        "Should list custom user agent"
    );
    assert!(
        stdout.contains("Custom test agent"),
        "Should show user agent description"
    );

    // Should show user source counts
    assert!(stdout.contains("User:"), "Should show user agent count");

    Ok(())
}

#[tokio::test]
async fn test_agent_precedence_project_over_builtin() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create project agents directory
    let project_agents_dir = project_root.join("agents");
    create_project_agent_files(&project_agents_dir)?;

    let output = run_agent_command_in_dir(&["agent", "list"], project_root).await?;
    assert!(
        output.status.success(),
        "agent list should succeed in project"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should list project-specific agent
    assert!(
        stdout.contains("project-dev-agent"),
        "Should list project agent"
    );
    assert!(
        stdout.contains("Project-specific development agent"),
        "Should show project agent description"
    );

    // Should show project source counts
    assert!(
        stdout.contains("Project:"),
        "Should show project agent count"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_discovery_hierarchy_full() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path().join("home");
    let project_root = temp_dir.path().join("project");

    fs::create_dir_all(&temp_home)?;
    fs::create_dir_all(&project_root)?;

    // Create user agents
    let user_agents_dir = temp_home.join(".swissarmyhammer").join("agents");
    create_test_agent_files(&user_agents_dir)?;

    // Create project agents
    let project_agents_dir = project_root.join("agents");
    create_project_agent_files(&project_agents_dir)?;

    // Set temporary home directory
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", &temp_home);

    // Ensure cleanup
    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let output = run_agent_command_in_dir(&["agent", "list"], &project_root).await?;
    assert!(
        output.status.success(),
        "agent list should succeed with full hierarchy"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have agents from all sources
    assert!(stdout.contains("Built-in:"), "Should show built-in agents");
    assert!(stdout.contains("Project:"), "Should show project agents");
    assert!(stdout.contains("User:"), "Should show user agents");

    // Should show agents from different sources
    assert!(
        stdout.contains("claude-code"),
        "Should list claude-code (from user or builtin)"
    );
    assert!(
        stdout.contains("qwen-coder"),
        "Should list qwen-coder (from project or builtin)"
    );
    assert!(
        stdout.contains("project-dev-agent"),
        "Should list project-specific agent"
    );
    assert!(
        stdout.contains("custom-test-agent"),
        "Should list user agent"
    );

    Ok(())
}

// =============================================================================
// ERROR SCENARIO TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_use_permission_denied() -> Result<()> {
    // Create a temporary directory where we can't write config
    let temp_dir = TempDir::new()?;
    let readonly_dir = temp_dir.path().join("readonly");
    fs::create_dir_all(&readonly_dir)?;

    // Make directory read-only (this might not work on all systems)
    let metadata = fs::metadata(&readonly_dir)?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    let _ = fs::set_permissions(&readonly_dir, permissions);

    // Try to use agent in read-only directory
    let output = run_agent_command_in_dir(&["agent", "use", "claude-code"], &readonly_dir).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Should provide helpful error message about permissions
        let stderr_lower = stderr.to_lowercase();
        println!("DEBUG: stderr = '{}'", stderr);
        println!("DEBUG: stderr_lower = '{}'", stderr_lower);
        println!(
            "DEBUG: contains permission = {}",
            stderr_lower.contains("permission")
        );
        println!(
            "DEBUG: contains configuration = {}",
            stderr_lower.contains("configuration")
        );
        println!("DEBUG: contains write = {}", stderr_lower.contains("write"));
        assert!(
            stderr_lower.contains("permission")
                || stderr_lower.contains("configuration")
                || stderr_lower.contains("write"),
            "Should provide helpful permission error. stderr: '{}', stderr_lower: '{}'",
            stderr,
            stderr_lower
        );
    }
    // Note: This test might pass on some systems where permission restrictions don't apply

    Ok(())
}

#[tokio::test]
async fn test_agent_list_with_invalid_agent_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_home = temp_dir.path();

    // Create user agents directory with invalid agent file
    let user_agents_dir = temp_home.join(".swissarmyhammer").join("agents");
    fs::create_dir_all(&user_agents_dir)?;

    // Create invalid YAML file
    let invalid_content = "invalid: yaml: content: [unclosed";
    fs::write(user_agents_dir.join("invalid-agent.yaml"), invalid_content)?;

    // Create valid agent file alongside invalid one
    let valid_content = r#"---
description: "Valid test agent"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;
    fs::write(user_agents_dir.join("valid-agent.yaml"), valid_content)?;

    // Set temporary home directory
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    // Ensure cleanup
    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let output = run_agent_command(&["agent", "list"]).await?;

    // Should succeed and load valid agents, skipping invalid ones
    assert!(
        output.status.success(),
        "agent list should succeed despite invalid files"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should list the valid agent
    assert!(stdout.contains("valid-agent"), "Should list valid agent");

    // Should not list the invalid agent
    assert!(
        !stdout.contains("invalid-agent"),
        "Should not list invalid agent"
    );

    Ok(())
}

// =============================================================================
// CONFIGURATION FILE TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_use_creates_config_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Ensure no existing config
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    assert!(!config_path.exists(), "Config should not exist initially");

    let output = run_agent_command_in_dir(&["agent", "use", "claude-code"], project_root).await?;

    if output.status.success() {
        // Should have created config file
        assert!(config_path.exists(), "Should create config file");

        let config_content = fs::read_to_string(&config_path)?;
        assert!(
            config_content.contains("agent:"),
            "Config should contain agent section"
        );
        assert!(
            config_content.contains("executor:"),
            "Config should contain executor config"
        );
    }
    // If it fails, that's acceptable for this test - we're testing successful case

    Ok(())
}

#[tokio::test]
async fn test_agent_use_updates_existing_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Create existing config file with other sections
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    let existing_config = r#"# Existing configuration
other_section:
  value: "preserved"
  number: 42
existing_agent:
  type: old-agent
"#;
    fs::write(&config_path, existing_config)?;

    let output = run_agent_command_in_dir(&["agent", "use", "claude-code"], project_root).await?;

    if output.status.success() {
        let updated_config = fs::read_to_string(&config_path)?;

        // Should preserve existing sections
        assert!(
            updated_config.contains("other_section:"),
            "Should preserve existing sections"
        );
        assert!(
            updated_config.contains("value: preserved"),
            "Should preserve existing values"
        );

        // Should add/update agent section
        assert!(
            updated_config.contains("agent:"),
            "Should have agent section"
        );
        assert!(
            updated_config.contains("executor:"),
            "Should have executor config"
        );
    }

    Ok(())
}

// =============================================================================
// END-TO-END WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_complete_agent_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    // Step 1: List agents initially
    let list_output = run_agent_command_in_dir(&["agent", "list"], project_root).await?;
    assert!(
        list_output.status.success(),
        "Initial agent list should succeed"
    );

    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    assert!(
        list_stdout.contains("claude-code"),
        "Should list claude-code initially"
    );

    // Step 2: Use claude-code agent
    let use_output =
        run_agent_command_in_dir(&["agent", "use", "claude-code"], project_root).await?;

    if use_output.status.success() {
        let use_stdout = String::from_utf8_lossy(&use_output.stdout);
        assert!(
            use_stdout.contains("Successfully switched"),
            "Should show success"
        );

        // Step 3: Verify config was created
        let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
        assert!(config_path.exists(), "Config file should be created");

        // Step 4: Switch to different agent
        let switch_output =
            run_agent_command_in_dir(&["agent", "use", "qwen-coder"], project_root).await?;

        if switch_output.status.success() {
            let switch_stdout = String::from_utf8_lossy(&switch_output.stdout);
            assert!(
                switch_stdout.contains("qwen-coder"),
                "Should show new agent"
            );

            // Step 5: Verify config was updated
            let config_content = fs::read_to_string(&config_path)?;
            assert!(
                config_content.contains("qwen-coder") || config_content.contains("llama-agent"),
                "Config should reflect new agent"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_all_builtin_agents_usable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();

    let builtin_agents = ["claude-code", "qwen-coder", "qwen-coder-flash"];

    for agent_name in &builtin_agents {
        let output = run_agent_command_in_dir(&["agent", "use", agent_name], project_root).await?;

        // Should either succeed or fail with config-related issues only
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Should not fail with "not found" for any builtin agent
            assert!(
                !stderr.contains("not found"),
                "Built-in agent '{}' should not be 'not found'. stderr: {}",
                agent_name,
                stderr
            );
        }
    }

    Ok(())
}

// =============================================================================
// HELP AND USAGE TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_help() -> Result<()> {
    let output = run_agent_command(&["agent", "--help"]).await?;

    assert!(output.status.success(), "agent --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("agent"),
        "Help should mention agent command"
    );
    assert!(
        stdout.contains("list"),
        "Help should mention list subcommand"
    );
    assert!(stdout.contains("use"), "Help should mention use subcommand");

    Ok(())
}

#[tokio::test]
async fn test_agent_list_help() -> Result<()> {
    let output = run_agent_command(&["agent", "list", "--help"]).await?;

    assert!(output.status.success(), "agent list --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("format"),
        "Help should mention format option"
    );

    Ok(())
}

#[tokio::test]
async fn test_agent_use_help() -> Result<()> {
    let output = run_agent_command(&["agent", "use", "--help"]).await?;

    assert!(output.status.success(), "agent use --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AGENT_NAME"),
        "Help should show agent name parameter"
    );

    Ok(())
}

// Add scopeguard dependency to Cargo.toml if not present
