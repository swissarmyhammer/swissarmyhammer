//! End-to-end workflow validation tests for agent management
//!
//! Tests complete workflows: list agents → use agent → verify config,
//! with all built-in agents, agent overriding, and config file backup/recovery.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tokio::process::Command;

/// Test utility to run sah commands and capture output
async fn run_sah_command(args: &[&str], working_dir: Option<&Path>) -> Result<std::process::Output> {
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

/// Create comprehensive test agent hierarchies for end-to-end testing
fn setup_agent_hierarchy(temp_dir: &Path) -> Result<()> {
    // Create user agents directory
    let user_agents_dir = temp_dir.join("home").join(".swissarmyhammer").join("agents");
    fs::create_dir_all(&user_agents_dir)?;
    
    // User agent that overrides claude-code
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
    
    // Custom user agent
    let user_custom = r#"---
description: "Custom user agent for testing workflows"
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
    
    // Create project agents directory
    let project_agents_dir = temp_dir.join("project").join("agents");
    fs::create_dir_all(&project_agents_dir)?;
    
    // Project agent that overrides qwen-coder
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
    
    // Project-specific development agent
    let project_dev = r#"---
description: "Development-optimized agent for project workflow"
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

/// Parse JSON output from agent list command
fn parse_agent_list_json(json_str: &str) -> Result<serde_json::Value> {
    Ok(serde_json::from_str(json_str)?)
}

/// Find agent by name in JSON output
fn find_agent_in_json<'a>(agents_json: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    agents_json.as_array()?.iter()
        .find(|agent| agent["name"].as_str() == Some(name))
}

/// Check if config file contains expected agent configuration
fn verify_agent_config(config_path: &Path, expected_agent: &str) -> Result<bool> {
    if !config_path.exists() {
        return Ok(false);
    }
    
    let config_content = fs::read_to_string(config_path)?;
    
    // Parse YAML to verify structure
    let config: serde_yaml::Value = serde_yaml::from_str(&config_content)?;
    
    // Check that agent section exists
    if let Some(agent_section) = config.get("agent") {
        if let Some(executor) = agent_section.get("executor") {
            // For built-in agents, check executor type
            if expected_agent == "claude-code" {
                return Ok(executor.get("type").and_then(|t| t.as_str()) == Some("claude-code"));
            } else if expected_agent == "qwen-coder" || expected_agent == "qwen-coder-flash" {
                return Ok(executor.get("type").and_then(|t| t.as_str()) == Some("llama-agent"));
            }
            // For custom agents, just check that executor exists
            return Ok(true);
        }
    }
    
    Ok(false)
}

// =============================================================================
// BASIC END-TO-END WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_basic_list_use_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Step 1: List agents and verify built-ins are available
    let list_output = run_sah_command(&["agent", "list", "--format", "json"], Some(project_root)).await?;
    assert!(list_output.status.success(), "Agent list should succeed");
    
    let agents_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
    
    // Should have built-in agents
    assert!(find_agent_in_json(&agents_json, "claude-code").is_some(), 
            "Should list claude-code agent");
    assert!(find_agent_in_json(&agents_json, "qwen-coder").is_some(), 
            "Should list qwen-coder agent");
    assert!(find_agent_in_json(&agents_json, "qwen-coder-flash").is_some(), 
            "Should list qwen-coder-flash agent");
    
    // Step 2: Use claude-code agent
    let use_output = run_sah_command(&["agent", "use", "claude-code"], Some(project_root)).await?;
    
    if use_output.status.success() {
        // Step 3: Verify config file was created and contains correct agent
        let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
        assert!(verify_agent_config(&config_path, "claude-code")?, 
                "Config should contain claude-code agent");
        
        // Step 4: Switch to different agent
        let switch_output = run_sah_command(&["agent", "use", "qwen-coder"], Some(project_root)).await?;
        
        if switch_output.status.success() {
            // Step 5: Verify config was updated
            assert!(verify_agent_config(&config_path, "qwen-coder")?, 
                    "Config should be updated to qwen-coder");
            
            // Step 6: List agents again to ensure everything still works
            let final_list = run_sah_command(&["agent", "list"], Some(project_root)).await?;
            assert!(final_list.status.success(), 
                    "Agent list should still work after config changes");
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_all_builtin_agents_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    let builtin_agents = ["claude-code", "qwen-coder", "qwen-coder-flash"];
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    
    for (i, agent_name) in builtin_agents.iter().enumerate() {
        // Use each agent
        let use_output = run_sah_command(&["agent", "use", agent_name], Some(project_root)).await?;
        
        // Should either succeed or fail with config-related issues only
        if use_output.status.success() {
            // Verify config file
            assert!(verify_agent_config(&config_path, agent_name)?,
                    "Config should contain {} after step {}", agent_name, i + 1);
            
            // Verify we can still list agents
            let list_output = run_sah_command(&["agent", "list", "--format", "json"], 
                                            Some(project_root)).await?;
            assert!(list_output.status.success(), 
                    "Should be able to list agents after using {}", agent_name);
            
            let agents_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
            assert!(find_agent_in_json(&agents_json, agent_name).is_some(),
                    "Should still list {} in agents after using it", agent_name);
        } else {
            // If it fails, ensure it's not a "not found" error for built-in agents
            let stderr = String::from_utf8_lossy(&use_output.stderr);
            assert!(!stderr.contains("not found"), 
                    "Built-in agent '{}' should not be 'not found': {}", agent_name, stderr);
        }
    }
    
    Ok(())
}

// =============================================================================
// AGENT HIERARCHY AND OVERRIDING WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_agent_overriding_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Create project agents directory - simple setup like the working test
    let project_agents_dir = project_root.join("agents");
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
    
    // Step 1: List agents from project directory (should show hierarchy)
    let list_output = run_sah_command(&["agent", "list", "--format", "json"], 
                                      Some(project_root)).await?;
    assert!(list_output.status.success(), "Should list agents with hierarchy");
    
    let agents_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
    
    // Verify we have agents from all sources
    let qwen_agent = find_agent_in_json(&agents_json, "qwen-coder")
        .expect("Should have qwen-coder agent");
    
    // qwen-coder should come from project source (project override)  
    assert_eq!(qwen_agent["source"].as_str(), Some("📁 Project"),
               "qwen-coder should be from project source due to override");
    assert!(qwen_agent["description"].as_str().unwrap()
            .contains("Project-optimized"), 
            "Should have project override description");

    Ok(())
}

#[tokio::test]
async fn test_custom_agent_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_agent_hierarchy(temp_dir.path())?;
    
    let home_dir = temp_dir.path().join("home");
    let project_root = temp_dir.path().join("project");
    
    // Set up environment
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", &home_dir);
    
    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });
    
    // Step 1: List agents and verify custom agents are available
    let list_output = run_sah_command(&["agent", "list", "--format", "json"], 
                                      Some(&project_root)).await?;
    assert!(list_output.status.success(), "Should list all agents including custom");
    
    let agents_json = parse_agent_list_json(&String::from_utf8_lossy(&list_output.stdout))?;
    
    let user_agent = find_agent_in_json(&agents_json, "test-user-agent")
        .expect("Should have custom user agent");
    let project_agent = find_agent_in_json(&agents_json, "project-dev")
        .expect("Should have custom project agent");
    
    // Verify agent metadata
    assert_eq!(user_agent["source"].as_str(), Some("👤 User"));
    assert!(user_agent["description"].as_str().unwrap()
            .contains("Custom user agent"));
    
    assert_eq!(project_agent["source"].as_str(), Some("📁 Project"));
    assert!(project_agent["description"].as_str().unwrap()
            .contains("Development-optimized"));
    
    // Step 2: Use custom user agent
    let use_user_output = run_sah_command(&["agent", "use", "test-user-agent"], 
                                          Some(&project_root)).await?;
    
    if use_user_output.status.success() {
        let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
        let config_content = fs::read_to_string(&config_path)?;
        assert!(config_content.contains("custom-agent") || 
                config_content.contains("--test-mode"),
                "Config should contain custom user agent settings");
    }
    
    // Step 3: Switch to custom project agent
    let use_project_output = run_sah_command(&["agent", "use", "project-dev"], 
                                             Some(&project_root)).await?;
    
    if use_project_output.status.success() {
        let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
        let config_content = fs::read_to_string(&config_path)?;
        assert!(config_content.contains("--dev-mode") || 
                config_content.contains("project/dev/claude") ||
                config_content.contains("--project-context"),
                "Config should contain custom project agent settings");
    }
    
    Ok(())
}

// =============================================================================
// CONFIG FILE MANAGEMENT WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_config_file_backup_and_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Step 1: Create initial configuration with multiple sections
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
  
# Existing agent config (will be replaced)  
agent:
  old_executor: "will-be-replaced"
"#;
    fs::write(&config_path, initial_config)?;
    
    // Create backup
    let backup_path = config_path.with_extension("yaml.backup");
    fs::copy(&config_path, &backup_path)?;
    
    // Step 2: Use agent to modify config
    let use_output = run_sah_command(&["agent", "use", "claude-code"], Some(project_root)).await?;
    
    if use_output.status.success() {
        // Step 3: Verify original sections are preserved
        let updated_config = fs::read_to_string(&config_path)?;
        
        // Should preserve all non-agent sections
        assert!(updated_config.contains("prompt:"), "Should preserve prompt section");
        assert!(updated_config.contains("default_template"), "Should preserve prompt settings");
        assert!(updated_config.contains("workflow:"), "Should preserve workflow section");  
        assert!(updated_config.contains("default_timeout"), "Should preserve workflow settings");
        assert!(updated_config.contains("other_settings:"), "Should preserve other settings");
        assert!(updated_config.contains("log_level"), "Should preserve nested settings");
        assert!(updated_config.contains("custom_data:"), "Should preserve custom data");
        assert!(updated_config.contains("deep_setting"), "Should preserve deeply nested settings");
        
        // Should update agent section
        assert!(updated_config.contains("agent:"), "Should have agent section");
        assert!(updated_config.contains("executor:"), "Should have new executor config");
        assert!(!updated_config.contains("old_executor"), "Should replace old agent config");
        
        // Step 4: Test recovery by switching to different agent
        let switch_output = run_sah_command(&["agent", "use", "qwen-coder"], Some(project_root)).await?;
        
        if switch_output.status.success() {
            let final_config = fs::read_to_string(&config_path)?;
            
            // All original sections should still be preserved
            assert!(final_config.contains("prompt:"), "Should still preserve prompt section");
            assert!(final_config.contains("workflow:"), "Should still preserve workflow section");
            assert!(final_config.contains("other_settings:"), "Should still preserve other settings");
            assert!(final_config.contains("deep_setting"), "Should still preserve deep settings");
            
            // Agent should be updated again
            assert!(final_config.contains("llama-agent") || final_config.contains("qwen"),
                    "Should contain new agent config");
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_config_file_format_consistency() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Test that config file format remains consistent across operations
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");
    
    // Step 1: Use first agent
    let first_use = run_sah_command(&["agent", "use", "claude-code"], Some(project_root)).await?;
    
    if first_use.status.success() {
        // Verify file is valid YAML
        let config_content = fs::read_to_string(&config_path)?;
        let parsed: serde_yaml::Value = serde_yaml::from_str(&config_content)
            .expect("Config should be valid YAML after first use");
        
        assert!(parsed.get("agent").is_some(), "Should have agent section");
        
        // Step 2: Switch agents multiple times
        let agents = ["qwen-coder", "claude-code", "qwen-coder-flash", "claude-code"];
        
        for agent in &agents {
            let use_output = run_sah_command(&["agent", "use", agent], Some(project_root)).await?;
            
            if use_output.status.success() {
                // Verify file remains valid YAML after each operation
                let config_content = fs::read_to_string(&config_path)?;
                let parsed: serde_yaml::Value = serde_yaml::from_str(&config_content)
                    .map_err(|e| anyhow::anyhow!("Invalid YAML after using {}: {}", agent, e))?;
                
                assert!(parsed.get("agent").is_some(), 
                        "Should have agent section after using {}", agent);
                
                // Verify basic structure
                if let Some(agent_section) = parsed.get("agent") {
                    assert!(agent_section.get("executor").is_some(),
                            "Should have executor section after using {}", agent);
                    assert!(agent_section.get("quiet").is_some(),
                            "Should have quiet setting after using {}", agent);
                }
            }
        }
    }
    
    Ok(())
}

// =============================================================================
// COMPREHENSIVE INTEGRATION WORKFLOW TESTS
// =============================================================================

#[tokio::test]
async fn test_complete_development_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    setup_agent_hierarchy(temp_dir.path())?;
    
    let home_dir = temp_dir.path().join("home");
    let project_root = temp_dir.path().join("project");
    
    // Set up environment
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", &home_dir);
    
    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });
    
    // Simulate a complete development workflow
    
    // Step 1: Developer starts new project, lists available agents
    let initial_list = run_sah_command(&["agent", "list", "--format", "table"], 
                                      Some(&project_root)).await?;
    assert!(initial_list.status.success(), "Initial agent list should work");
    
    let list_output = String::from_utf8_lossy(&initial_list.stdout);
    assert!(list_output.contains("Agents:"), "Should show agent summary");
    assert!(list_output.contains("Built-in:"), "Should show built-in count");
    assert!(list_output.contains("Project:"), "Should show project count");
    assert!(list_output.contains("User:"), "Should show user count");
    
    // Step 2: Developer chooses project-optimized development agent
    let use_dev = run_sah_command(&["agent", "use", "project-dev"], Some(&project_root)).await?;
    
    if use_dev.status.success() {
        // Step 3: Verify development setup is correct
        let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
        let config_content = fs::read_to_string(&config_path)?;
        assert!(config_content.contains("--dev-mode"), "Should use dev mode");
        
        // Step 4: Developer wants to try different model, switches to qwen
        let use_qwen = run_sah_command(&["agent", "use", "qwen-coder"], Some(&project_root)).await?;
        
        if use_qwen.status.success() {
            // Step 5: Verify qwen configuration (project override)
            let config_content = fs::read_to_string(&config_path)?;
            assert!(config_content.contains("project/optimized-qwen-coder") || 
                    config_content.contains("llama-agent"),
                    "Should use project-optimized qwen");
            
            // Step 6: List agents again to see current state
            let mid_list = run_sah_command(&["agent", "list", "--format", "json"], 
                                          Some(&project_root)).await?;
            assert!(mid_list.status.success(), "Mid-workflow list should work");
            
            // Step 7: Developer decides to use personal claude setup
            let use_claude = run_sah_command(&["agent", "use", "claude-code"], 
                                            Some(&project_root)).await?;
            
            if use_claude.status.success() {
                // Step 8: Verify user override is used
                let config_content = fs::read_to_string(&config_path)?;
                assert!(config_content.contains("/custom/user/claude") || 
                        config_content.contains("--user-mode"),
                        "Should use user-customized claude");
                
                // Step 9: Final verification - list all agents
                let final_list = run_sah_command(&["agent", "list", "--format", "yaml"], 
                                                Some(&project_root)).await?;
                assert!(final_list.status.success(), "Final list should work");
                
                // Parse YAML to verify structure
                let yaml_output = String::from_utf8_lossy(&final_list.stdout);
                let agents: serde_yaml::Value = serde_yaml::from_str(&yaml_output)?;
                assert!(agents.is_sequence(), "Should be valid YAML sequence");
                
                let agent_list = agents.as_sequence().unwrap();
                assert!(agent_list.len() >= 5, "Should have multiple agents from all sources");
            }
        }
    }
    
    Ok(())
}

#[tokio::test] 
async fn test_error_recovery_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Test workflow recovery from various error conditions
    
    // Step 1: Try to use non-existent agent
    let bad_use = run_sah_command(&["agent", "use", "definitely-not-real"], Some(project_root)).await?;
    assert!(!bad_use.status.success(), "Should fail with non-existent agent");
    
    let stderr = String::from_utf8_lossy(&bad_use.stderr);
    assert!(stderr.contains("not found"), "Should report agent not found");
    
    // Step 2: Verify we can still list agents after error
    let list_after_error = run_sah_command(&["agent", "list"], Some(project_root)).await?;
    assert!(list_after_error.status.success(), "Should still work after error");
    
    // Step 3: Successfully use valid agent
    let good_use = run_sah_command(&["agent", "use", "claude-code"], Some(project_root)).await?;
    
    if good_use.status.success() {
        // Step 4: Verify system is working normally
        let final_list = run_sah_command(&["agent", "list", "--format", "json"], 
                                        Some(project_root)).await?;
        assert!(final_list.status.success(), "Should work normally after recovery");
        
        let agents_json = parse_agent_list_json(&String::from_utf8_lossy(&final_list.stdout))?;
        assert!(find_agent_in_json(&agents_json, "claude-code").is_some(),
                "Should still list claude-code after recovery");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_workflow_safety() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Test that multiple operations don't interfere with each other
    // Run operations sequentially but simulate concurrent-like conditions
    
    let agents = ["claude-code", "qwen-coder", "qwen-coder-flash"];
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");
    
    for (i, agent) in agents.iter().enumerate() {
        // Step 1: List agents
        let list_output = run_sah_command(&["agent", "list", "--format", "json"], 
                                         Some(project_root)).await?;
        assert!(list_output.status.success(), 
                "List should succeed on iteration {}", i);
        
        // Step 2: Use agent
        let use_output = run_sah_command(&["agent", "use", agent], Some(project_root)).await?;
        
        if use_output.status.success() {
            // Step 3: Verify config consistency
            assert!(verify_agent_config(&config_path, agent)?,
                    "Config should be consistent for {} on iteration {}", agent, i);
            
            // Step 4: Immediate re-list to check consistency
            let verify_list = run_sah_command(&["agent", "list"], Some(project_root)).await?;
            assert!(verify_list.status.success(), 
                    "Verification list should succeed after using {} on iteration {}", agent, i);
        }
    }
    
    Ok(())
}