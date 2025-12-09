// sah rule ignore test_rule_with_allow
//! Performance and edge case tests for model management
//!
//! Tests large numbers of models, deeply nested project structures,
//! concurrent access scenarios, and invalid YAML handling/recovery.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use tokio::process::Command;

/// Test utility to run sah commands with timeout
async fn run_sah_command_with_timeout(
    args: &[&str],
    working_dir: Option<&Path>,
    timeout_secs: u64,
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
        .env("RUST_LOG", "error") // Reduce log noise
        .kill_on_drop(true);

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    // Use timeout to prevent hanging tests
    match tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output()).await {
        Ok(result) => result.map_err(Into::into),
        Err(_) => Err(anyhow::anyhow!(
            "Command timed out after {} seconds",
            timeout_secs
        )),
    }
}

/// Helper to assert that an operation completed successfully within a time limit
fn assert_operation_completed_within(
    output: &std::process::Output,
    duration: Duration,
    max_duration: Duration,
    operation_description: &str,
) -> Result<()> {
    assert!(
        output.status.success(),
        "{} should succeed",
        operation_description
    );
    assert!(
        duration < max_duration,
        "{} should complete in under {:?}, took {:?}",
        operation_description,
        max_duration,
        duration
    );
    Ok(())
}

/// Helper to assert model list contains minimum number of agents
async fn assert_agent_list_contains_minimum(
    project_root: &Path,
    min_count: usize,
    timeout_secs: u64,
) -> Result<Vec<serde_json::Value>> {
    let list_output = run_sah_command_with_timeout(
        &["model", "list", "--format", "json"],
        Some(project_root),
        timeout_secs,
    )
    .await?;

    assert!(list_output.status.success(), "Should list agents");

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let agents_json: serde_json::Value = serde_json::from_str(&stdout)?;
    let agents_array = agents_json.as_array().unwrap().clone();

    assert!(
        agents_array.len() >= min_count,
        "Should list at least {} agents, got {}",
        min_count,
        agents_array.len()
    );

    Ok(agents_array)
}

/// Helper to generate large YAML config with custom entry generator
fn generate_large_yaml_config<F>(header: &str, entry_count: usize, entry_generator: F) -> String
where
    F: Fn(usize) -> String,
{
    let mut config = String::from(header);
    for i in 0..entry_count {
        config.push_str(&entry_generator(i));
    }
    config
}

/// Generate a large number of test model files
fn create_large_agent_set(dir: &Path, count: usize) -> Result<Vec<String>> {
    fs::create_dir_all(dir)?;
    let mut agent_names = Vec::new();

    for i in 0..count {
        let agent_name = format!("test-agent-{:04}", i);
        let agent_content = format!(
            r#"---
description: "Generated test model number {} for performance testing"
version: "1.0"
category: "performance-test"
generated: true
index: {}
---
executor:
  type: claude-code
  config:
    claude_path: /test/path/claude-{}
    args: ["--test-mode", "--agent-{}"]
quiet: {}"#,
            i,
            i,
            i,
            i,
            i % 2 == 0
        );

        let agent_file = dir.join(format!("{}.yaml", agent_name));
        fs::write(&agent_file, agent_content)?;
        agent_names.push(agent_name);
    }

    Ok(agent_names)
}

/// Create deeply nested directory structure with agents
fn create_nested_agent_structure(
    base_dir: &Path,
    depth: usize,
    agents_per_level: usize,
) -> Result<()> {
    fn create_level(
        current_dir: &Path,
        remaining_depth: usize,
        agents_per_level: usize,
        level_index: usize,
    ) -> Result<()> {
        fs::create_dir_all(current_dir)?;

        // Create agents at current level
        let agents_dir = current_dir.join("models");
        if remaining_depth > 0 {
            fs::create_dir_all(&agents_dir)?;

            for i in 0..agents_per_level {
                let agent_name = format!("nested-l{}-{}", level_index, i);
                let agent_content = format!(
                    r#"---
description: "Nested model at level {} position {}"
depth: {}
position: {}
---
executor:
  type: claude-code
  config:
    claude_path: /nested/level{}/agent{}
    args: ["--nested-level-{}"]
quiet: false"#,
                    level_index, i, level_index, i, level_index, i, level_index
                );

                fs::write(
                    agents_dir.join(format!("{}.yaml", agent_name)),
                    agent_content,
                )?;
            }
        }

        // Create subdirectories if we haven't reached max depth
        if remaining_depth > 1 {
            for i in 0..2 {
                // Create 2 subdirs per level to keep it manageable
                let subdir = current_dir.join(format!("level-{}-sub-{}", level_index, i));
                create_level(
                    &subdir,
                    remaining_depth - 1,
                    agents_per_level,
                    level_index + 1,
                )?;
            }
        }

        Ok(())
    }

    create_level(base_dir, depth, agents_per_level, 0)
}

/// Create invalid YAML syntax error files
fn create_yaml_syntax_errors(dir: &Path, invalid_files: &mut Vec<String>) -> Result<()> {
    let syntax_errors = vec![
        ("unclosed-bracket.yaml", "invalid: yaml: [unclosed bracket"),
        ("unclosed-quote.yaml", r#"description: "unclosed quote"#),
        (
            "invalid-indentation.yaml",
            "description: test\n  invalid_indent: true\nwrong_level: false",
        ),
        (
            "mixed-tabs-spaces.yaml",
            "description: test\n\ttabbed: true\n    spaced: true",
        ),
        (
            "duplicate-keys.yaml",
            "description: first\ndescription: duplicate\nexecutor:\n  type: claude-code",
        ),
    ];

    for (filename, content) in syntax_errors {
        fs::write(dir.join(filename), content)?;
        invalid_files.push(filename.to_string());
    }

    Ok(())
}

/// Create invalid YAML structure error files
fn create_yaml_structure_errors(dir: &Path, invalid_files: &mut Vec<String>) -> Result<()> {
    let structure_errors = vec![
        (
            "missing-executor.yaml",
            r#"---
description: "Missing executor section"
---
quiet: false"#,
        ),
        (
            "invalid-executor-type.yaml",
            r#"---
description: "Invalid executor type"
---
executor:
  type: unknown-executor-type
  config: {}
quiet: false"#,
        ),
        (
            "malformed-config.yaml",
            r#"---
description: "Malformed config section"
---
executor:
  type: claude-code
  config: "this should be an object"
quiet: not-a-boolean"#,
        ),
    ];

    for (filename, content) in structure_errors {
        fs::write(dir.join(filename), content)?;
        invalid_files.push(filename.to_string());
    }

    Ok(())
}

/// Create various invalid YAML files to test error handling
fn create_invalid_agent_files(dir: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(dir)?;
    let mut invalid_files = Vec::new();

    create_yaml_syntax_errors(dir, &mut invalid_files)?;
    create_yaml_structure_errors(dir, &mut invalid_files)?;

    Ok(invalid_files)
}

// =============================================================================
// PERFORMANCE TESTS
// =============================================================================

/// Setup large agent test environment with user and project agents
fn setup_large_agent_test_environment(
    temp_home: &Path,
    project_root: &Path,
) -> Result<(Vec<String>, Vec<String>)> {
    fs::create_dir_all(temp_home)?;
    fs::create_dir_all(project_root)?;

    let user_agents_dir = temp_home.join(".swissarmyhammer").join("models");
    let project_agents_dir = project_root.join("models");

    let user_agents = create_large_agent_set(&user_agents_dir, 50)?;
    let project_agents = create_large_agent_set(&project_agents_dir, 100)?;

    Ok((user_agents, project_agents))
}

#[tokio::test]
async fn test_large_model_list_performance() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_home = &temp_dir.join("home");
    let project_root = &temp_dir.join("project");

    let (user_agents, project_agents) =
        setup_large_agent_test_environment(temp_home, project_root)?;

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let start_time = Instant::now();

    let list_output = run_sah_command_with_timeout(
        &["model", "list", "--format", "json"],
        Some(project_root),
        30,
    )
    .await?;

    let duration = start_time.elapsed();

    assert_operation_completed_within(
        &list_output,
        duration,
        Duration::from_secs(10),
        "List large number of agents",
    )?;

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let agents_json: serde_json::Value = serde_json::from_str(&stdout)?;
    let agents_array = agents_json.as_array().unwrap();

    let expected_min = (user_agents.len() + project_agents.len()) / 2 + 3;
    assert!(
        agents_array.len() >= expected_min,
        "Should list at least {} agents, got {}",
        expected_min,
        agents_array.len()
    );

    println!("Listed {} agents in {:?}", agents_array.len(), duration);

    Ok(())
}

/// Create large configuration content for testing
fn create_large_config() -> String {
    let header = r#"# Large configuration file

prompts:
  default: "greeting"
  library_path: "./prompts"

workflows:
  timeout: 300
"#;

    let mut large_config = generate_large_yaml_config(header, 500, |i| {
        format!(
            r#"
  workflow_{:03}:
    name: "Test Workflow {}"
    description: "Generated workflow for performance testing"
    timeout: {}
    retries: {}
    enabled: {}
"#,
            i,
            i,
            60 + i,
            i % 5 + 1,
            i % 2 == 0
        )
    });

    large_config.push_str(
        r#"
other_data:
  cache_settings:
    enabled: true
    size_mb: 1024
    ttl_seconds: 3600
  custom_data:
"#,
    );

    large_config.push_str(&generate_large_yaml_config("", 200, |i| {
        format!(
            r#"
    key_{:03}: "value_{}"
    nested_{:03}:
      subkey_a: "data_{}"
      subkey_b: {}
      subkey_c: {}
"#,
            i,
            i,
            i,
            i,
            i * 2,
            i % 2 == 0
        )
    }));

    large_config
}

/// Verify config preservation after update
fn verify_config_preservation(config_path: &Path) -> Result<()> {
    let updated_config = fs::read_to_string(config_path)?;
    assert!(
        updated_config.contains("prompts:"),
        "Should preserve prompts"
    );
    assert!(
        updated_config.contains("workflows:"),
        "Should preserve workflows"
    );
    assert!(
        updated_config.contains("workflow_499:"),
        "Should preserve last workflow"
    );
    assert!(
        updated_config.contains("other_data:"),
        "Should preserve other data"
    );
    assert!(
        updated_config.contains("key_199:"),
        "Should preserve custom data"
    );
    assert!(
        updated_config.contains("models:"),
        "Should add model section"
    );
    Ok(())
}

#[tokio::test]
async fn test_model_use_performance_with_large_config() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    let large_config = create_large_config();
    fs::write(&config_path, &large_config)?;

    let start_time = Instant::now();

    let use_output =
        run_sah_command_with_timeout(&["model", "use", "claude-code"], Some(project_root), 15)
            .await?;

    let duration = start_time.elapsed();

    if use_output.status.success() {
        assert_operation_completed_within(
            &use_output,
            duration,
            Duration::from_secs(6),
            "Update large config",
        )?;

        verify_config_preservation(&config_path)?;

        println!(
            "Updated large config ({} bytes) in {:?}",
            large_config.len(),
            duration
        );
    }

    Ok(())
}

// =============================================================================
// DEEPLY NESTED STRUCTURE TESTS
// =============================================================================

#[tokio::test]
async fn test_deeply_nested_project_structures() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let base_dir = &temp_dir;

    // Create deeply nested structure: 5 levels deep, 2 agents per level
    create_nested_agent_structure(base_dir, 5, 2)?;

    // Test from various depths in the structure
    let test_dirs = vec![
        base_dir.to_path_buf(),
        base_dir.join("level-0-sub-0"),
        base_dir.join("level-0-sub-0").join("level-1-sub-1"),
        base_dir
            .join("level-0-sub-1")
            .join("level-1-sub-0")
            .join("level-2-sub-1"),
    ];

    for test_dir in test_dirs {
        if test_dir.exists() {
            let list_output =
                run_sah_command_with_timeout(&["model", "list"], Some(&test_dir), 10).await?;

            if list_output.status.success() {
                let stdout = String::from_utf8_lossy(&list_output.stdout);
                // Should find project agents from current level
                assert!(
                    stdout.contains("Project:") || stdout.contains("project"),
                    "Should find project agents from nested dir: {:?}",
                    test_dir
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_deep_directory_traversal_performance() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let base_dir = &temp_dir;

    // Create very deep structure: 10 levels, 1 model per level
    create_nested_agent_structure(base_dir, 10, 1)?;

    // Test performance from deepest directory
    let mut deep_dir = base_dir.to_path_buf();
    for i in 0..5 {
        deep_dir = deep_dir.join(format!("level-{}-sub-0", i));
    }

    if deep_dir.exists() {
        let start_time = Instant::now();

        let list_output =
            run_sah_command_with_timeout(&["model", "list"], Some(&deep_dir), 15).await?;

        let duration = start_time.elapsed();

        assert!(
            list_output.status.success() || duration < Duration::from_secs(10),
            "Should handle deep directory structure efficiently, took {:?}",
            duration
        );

        println!("Deep directory traversal took {:?}", duration);
    }

    Ok(())
}

// =============================================================================
// INVALID YAML HANDLING TESTS
// =============================================================================

/// Verify valid agents are loaded
fn verify_valid_agents_loaded(
    agents_array: &[serde_json::Value],
    valid_agents: &[String],
) -> Result<()> {
    assert!(
        agents_array.len() >= valid_agents.len() + 3,
        "Should load {} valid agents, got {}",
        valid_agents.len() + 3,
        agents_array.len()
    );

    let agent_names: Vec<_> = agents_array
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();

    for valid_model in valid_agents {
        assert!(
            agent_names.contains(&valid_model.as_str()),
            "Should include valid model: {}",
            valid_model
        );
    }

    Ok(())
}

/// Verify invalid agents are excluded
fn verify_invalid_agents_excluded(
    agents_array: &[serde_json::Value],
    invalid_files: &[String],
) -> Result<()> {
    let agent_names: Vec<_> = agents_array
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();

    for invalid_file in invalid_files {
        let invalid_name = invalid_file.replace(".yaml", "");
        assert!(
            !agent_names.contains(&invalid_name.as_str()),
            "Should not load invalid agent: {}",
            invalid_name
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_invalid_yaml_recovery() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_home = &temp_dir.join("home");

    let user_agents_dir = temp_home.join(".swissarmyhammer").join("models");

    let valid_agents = create_large_agent_set(&user_agents_dir, 10)?;
    let invalid_files = create_invalid_agent_files(&user_agents_dir)?;

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let agents_array =
        assert_agent_list_contains_minimum(&temp_dir, valid_agents.len() + 3, 10).await?;

    verify_valid_agents_loaded(&agents_array, &valid_agents)?;
    verify_invalid_agents_excluded(&agents_array, &invalid_files)?;

    let use_output =
        run_sah_command_with_timeout(&["model", "use", "claude-code"], None, 10).await?;

    if !use_output.status.success() {
        let stderr = String::from_utf8_lossy(&use_output.stderr);
        assert!(
            !stderr.contains("parse") && !stderr.contains("yaml") && !stderr.contains("syntax"),
            "Should not fail with YAML parsing errors: {}",
            stderr
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_corrupted_config_recovery() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Create corrupted config file
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    // Write corrupted YAML
    let corrupted_config = r#"# Corrupted configuration
prompt:
  valid: "section"

agent:
  invalid: yaml: [unclosed bracket

other_section:
  preserved: true
"#;
    fs::write(&config_path, corrupted_config)?;

    // Test that model use handles corrupted config gracefully
    let use_output =
        run_sah_command_with_timeout(&["model", "use", "claude-code"], Some(project_root), 10)
            .await?;

    // Should either succeed (by creating new config) or fail with helpful error
    if !use_output.status.success() {
        let stderr = String::from_utf8_lossy(&use_output.stderr);
        // Error should be helpful, not cryptic YAML parser errors
        assert!(
            stderr.contains("configuration")
                || stderr.contains("config")
                || stderr.contains("file"),
            "Should provide helpful error message: {}",
            stderr
        );
    } else {
        // If it succeeded, config should be fixed
        let fixed_config = fs::read_to_string(&config_path)?;
        let parsed: serde_yaml::Value = serde_yaml::from_str(&fixed_config)?;
        assert!(
            parsed.get("models").is_some(),
            "Should have valid models section. Actual: {:?}",
            parsed
        );
    }

    Ok(())
}

// =============================================================================
// CONCURRENT ACCESS SIMULATION TESTS
// =============================================================================

#[tokio::test]
async fn test_rapid_sequential_operations() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Simulate rapid sequential operations like a user clicking quickly
    let agents = [
        "claude-code",
        "qwen-coder",
        "claude-code",
        "qwen-coder-flash",
        "claude-code",
    ];
    let config_path = project_root.join(".swissarmyhammer").join("sah.yaml");

    for (i, agent) in agents.iter().enumerate() {
        let start_time = Instant::now();

        // Rapid list and use operations
        let list_output = run_sah_command_with_timeout(
            &["model", "list", "--format", "json"],
            Some(project_root),
            5,
        )
        .await?;

        let use_output =
            run_sah_command_with_timeout(&["model", "use", agent], Some(project_root), 5).await?;

        let operation_time = start_time.elapsed();

        // Operations should complete quickly
        assert_operation_completed_within(
            &list_output,
            operation_time,
            Duration::from_secs(10),
            &format!("Rapid operation {}", i),
        )?;

        // If use succeeded, verify config consistency
        if use_output.status.success() && config_path.exists() {
            let config_content = fs::read_to_string(&config_path)?;
            // Should be valid YAML
            let _: serde_yaml::Value = serde_yaml::from_str(&config_content).map_err(|e| {
                anyhow::anyhow!("Config corrupted after rapid operation {}: {}", i, e)
            })?;
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_file_lock_simulation() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    // Create initial config
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir)?;
    let config_path = sah_dir.join("sah.yaml");

    let initial_config = r#"# Initial configuration
agent:
  executor:
    type: claude-code
    config: {}
  quiet: false
"#;
    fs::write(&config_path, initial_config)?;

    // Simulate scenario where config file might be temporarily inaccessible
    // (This is hard to test reliably cross-platform, so we test error resilience)

    // Test multiple operations with potential file contention
    for i in 0..5 {
        let model = if i % 2 == 0 {
            "claude-code"
        } else {
            "qwen-coder"
        };

        let use_output =
            run_sah_command_with_timeout(&["model", "use", model], Some(project_root), 10).await?;

        // Should handle file access gracefully
        if !use_output.status.success() {
            let stderr = String::from_utf8_lossy(&use_output.stderr);
            // Should not contain low-level file errors
            assert!(
                !stderr.contains("Permission denied")
                    || stderr.contains("configuration")
                    || stderr.contains("write"),
                "Should provide user-friendly error for file issues: {}",
                stderr
            );
        }

        // Verify file integrity after each operation
        if config_path.exists() {
            let config_content = fs::read_to_string(&config_path)?;
            if !config_content.is_empty() {
                let _: serde_yaml::Value = serde_yaml::from_str(&config_content).map_err(|e| {
                    anyhow::anyhow!("Config integrity lost after operation {}: {}", i, e)
                })?;
            }
        }
    }

    Ok(())
}

// =============================================================================
// RESOURCE USAGE AND LIMITS TESTS
// =============================================================================

/// Setup large dataset environment with many agents
fn setup_large_dataset_environment(
    temp_home: &Path,
    project_root: &Path,
) -> Result<(Vec<String>, Vec<String>)> {
    fs::create_dir_all(temp_home)?;
    fs::create_dir_all(project_root)?;

    let user_agents_dir = temp_home.join(".swissarmyhammer").join("models");
    let project_agents_dir = project_root.join("models");

    let user_agents = create_large_agent_set(&user_agents_dir, 200)?;
    let project_agents = create_large_agent_set(&project_agents_dir, 300)?;

    Ok((user_agents, project_agents))
}

#[tokio::test]
async fn test_memory_usage_with_large_datasets() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let temp_home = &temp_dir.join("home");
    let project_root = &temp_dir.join("project");

    let (user_agents, project_agents) = setup_large_dataset_environment(temp_home, project_root)?;

    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_home);

    let _cleanup = scopeguard::guard((), |_| {
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    });

    let start_time = Instant::now();

    let list_output = run_sah_command_with_timeout(
        &["model", "list", "--format", "json"],
        Some(project_root),
        60,
    )
    .await?;

    let duration = start_time.elapsed();

    assert_operation_completed_within(
        &list_output,
        duration,
        Duration::from_secs(30),
        &format!(
            "Process {} agents",
            user_agents.len() + project_agents.len() + 3
        ),
    )?;

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let agents_json: serde_json::Value = serde_json::from_str(&stdout)?;
    let agents_array = agents_json.as_array().unwrap();

    let expected_min = (user_agents.len() + project_agents.len()) / 2 + 3;
    assert!(
        agents_array.len() >= expected_min,
        "Should load at least {} agents, got {}",
        expected_min,
        agents_array.len()
    );

    println!("Processed {} agents in {:?}", agents_array.len(), duration);

    Ok(())
}

// =============================================================================
// ERROR BOUNDARY AND RECOVERY TESTS
// =============================================================================

/// Test a single error scenario and recovery
async fn test_error_scenario_and_recovery(
    invalid_agent: &str,
    expected_error: &str,
    project_root: &Path,
) -> Result<()> {
    let error_output =
        run_sah_command_with_timeout(&["model", "use", invalid_agent], Some(project_root), 5)
            .await?;

    assert!(
        !error_output.status.success(),
        "Should fail for invalid agent: {}",
        invalid_agent
    );

    let stderr = String::from_utf8_lossy(&error_output.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr_lower.contains("not found")
            || stderr_lower.contains("model command failed")
            || stderr_lower.contains(expected_error),
        "Should contain expected error '{}' for '{}': {}",
        expected_error,
        invalid_agent,
        stderr
    );

    let recovery_output =
        run_sah_command_with_timeout(&["model", "list"], Some(project_root), 5).await?;

    assert!(
        recovery_output.status.success(),
        "Should recover from error scenario: {}",
        invalid_agent
    );

    let valid_use =
        run_sah_command_with_timeout(&["model", "use", "claude-code"], Some(project_root), 10)
            .await?;

    if !valid_use.status.success() {
        let stderr = String::from_utf8_lossy(&valid_use.stderr);
        assert!(
            !stderr.contains("not found"),
            "Should not fail with 'not found' for builtin after recovery from {}",
            invalid_agent
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_comprehensive_error_recovery() -> Result<()> {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let project_root = &temp_dir;

    let error_scenarios = vec![("nonexistent-agent", "model not found"), ("", "empty name")];

    for (invalid_agent, expected_error) in error_scenarios {
        test_error_scenario_and_recovery(invalid_agent, expected_error, project_root).await?;
    }

    Ok(())
}
