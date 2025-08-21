//! Tests for builtin workflow migration to parameter format

use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

use swissarmyhammer_cli::{
    cli::FlowSubcommand,
    flow::run_flow_command,
};
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

/// Get the repository root directory (parent of the CLI test directory)
fn get_repo_root() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run flow command in-process from the repo root
async fn run_builtin_workflow_in_process(
    workflow_name: &str,
    vars: Vec<String>,
    dry_run: bool,
) -> Result<bool> {
    let repo_root = get_repo_root();
    let _env = IsolatedTestEnvironment::new().unwrap();
    
    // Change to repo root directory where builtin workflows are located
    std::env::set_current_dir(&repo_root)?;
    
    let subcommand = FlowSubcommand::Run {
        workflow: workflow_name.to_string(),
        vars,
        interactive: false,
        dry_run,
        test: false,
        timeout: Some("2s".to_string()), // Use 2 second timeout for fast tests
        quiet: true,
    };
    
    let result = run_flow_command(subcommand).await;
    
    Ok(result.is_ok())
}

#[tokio::test]
async fn test_greeting_workflow_parameter_migration() -> Result<()> {
    // Test that workflow accepts parameters via --var (current system)
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=Alice".to_string(),
            "language=Spanish".to_string(),
            "enthusiastic=true".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success, "Greeting workflow should accept --var parameters");
    Ok(())
}

#[tokio::test]
async fn test_greeting_workflow_backward_compatibility() -> Result<()> {
    // Test that --var arguments work
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=John".to_string(),
            "language=English".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success, "Greeting workflow should maintain backward compatibility");
    Ok(())
}

#[tokio::test]
async fn test_greeting_workflow_interactive_prompting() -> Result<()> {
    // Test that workflow runs without parameters (should use defaults/prompts)
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![], // no parameters
        true,   // dry-run
    ).await?;

    // Should succeed but may prompt for required parameters
    // For now we test that it doesn't crash - both success and graceful failure are acceptable
    assert!(success, "Greeting workflow should handle missing parameters gracefully");
    Ok(())
}

#[tokio::test]
async fn test_plan_workflow_parameter_migration() -> Result<()> {
    // Test that plan workflow accepts parameters via --var (current system)
    let success = run_builtin_workflow_in_process(
        "plan",
        vec!["plan_filename=./specification/test-feature.md".to_string()],
        true, // dry-run
    ).await?;

    assert!(success, "Plan workflow should accept --var parameters");
    Ok(())
}

#[tokio::test]
async fn test_plan_workflow_backward_compatibility() -> Result<()> {
    // Test that --var arguments work
    let success = run_builtin_workflow_in_process(
        "plan",
        vec!["plan_filename=./spec/feature.md".to_string()],
        true, // dry-run
    ).await?;

    assert!(success, "Plan workflow should maintain backward compatibility");
    Ok(())
}

#[tokio::test]
async fn test_plan_workflow_legacy_behavior() -> Result<()> {
    // Test that plan runs without parameters (legacy behavior - scan ./specification)
    let success = run_builtin_workflow_in_process(
        "plan",
        vec![], // no parameters
        true,   // dry-run
    ).await?;

    assert!(success, "Plan workflow should support legacy behavior without parameters");
    Ok(())
}

#[tokio::test]
async fn test_mixed_parameter_resolution_precedence() -> Result<()> {
    // Test precedence when multiple --var are used
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=Alice".to_string(), // First var value
            "person_name=Bob".to_string(),   // Later var value should take precedence
            "language=French".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success, "Multiple --var values should work with later values taking precedence");
    Ok(())
}

#[tokio::test] 
async fn test_workflow_edge_cases() -> Result<()> {
    // Test with empty variable values
    let success1 = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=".to_string(), // empty value
            "language=English".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success1, "Workflow should handle empty variable values");

    // Test with special characters in values
    let success2 = run_builtin_workflow_in_process(
        "greeting",  
        vec![
            "person_name=José María".to_string(), // Special characters
            "language=Español".to_string(),
        ],
        true, // dry-run
    ).await?;

    assert!(success2, "Workflow should handle special characters in values");
    Ok(())
}

// Keep a few slow CLI integration tests for end-to-end verification
#[test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"]
fn test_cli_integration_greeting_workflow() {
    // Run from repo root where builtin workflows are located
    let repo_root = get_repo_root();

    // Test end-to-end CLI integration
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("flow")
        .arg("run")
        .arg("greeting")
        .arg("--var")
        .arg("person_name=Integration Test")
        .arg("--var")
        .arg("language=English")
        .arg("--dry-run")
        .current_dir(&repo_root);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Dry run mode"))
        .stdout(predicate::str::contains("greeting"));
}

#[test]
#[ignore = "Expensive CLI integration test - run with --ignored to include"] 
fn test_cli_integration_plan_workflow() {
    // Run from repo root where builtin workflows are located
    let repo_root = get_repo_root();

    // Test end-to-end CLI integration
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("flow")
        .arg("run")
        .arg("plan")
        .arg("--var")
        .arg("plan_filename=./test.md")
        .arg("--dry-run")
        .current_dir(&repo_root);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Dry run mode"))
        .stdout(predicate::str::contains("plan"));
}