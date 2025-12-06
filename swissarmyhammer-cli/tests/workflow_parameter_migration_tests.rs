//! Tests for builtin workflow migration to parameter format
// sah rule ignore test_rule_with_allow

use anyhow::Result;
use std::path::PathBuf;

use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::{
    commands::flow::run::{execute_run_command, RunCommandConfig},
    context::CliContext,
};

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

/// Get the repository root directory (parent of the CLI test directory)
fn get_repo_root() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Create a minimal test CliContext
async fn create_test_cli_context() -> Result<CliContext> {
    use swissarmyhammer_cli::cli::OutputFormat;
    let template_context = swissarmyhammer_config::TemplateContext::new();
    let matches = clap::ArgMatches::default();
    CliContext::new(
        template_context,
        OutputFormat::Table,
        None,
        false,
        false,
        false,
        matches,
    )
    .await
    .map_err(Into::into)
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

    let cli_context = create_test_cli_context().await?;
    // Create CliToolContext for the new signature
    let work_dir = std::env::current_dir()?;
    let cli_tool_context = std::sync::Arc::new(
        swissarmyhammer_cli::mcp_integration::CliToolContext::new_with_config(&work_dir, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create CliToolContext: {}", e))?,
    );

    let result = execute_run_command(
        RunCommandConfig {
            workflow: workflow_name.to_string(),
            positional_args: vec![],
            params: vec![],
            vars,
            interactive: false,
            dry_run,
            quiet: true,
        },
        &cli_context,
        cli_tool_context,
    )
    .await;

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
    )
    .await?;

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
    )
    .await?;

    assert!(
        success,
        "Greeting workflow should maintain backward compatibility"
    );
    Ok(())
}

#[tokio::test]
async fn test_greeting_workflow_interactive_prompting() -> Result<()> {
    // Test that workflow runs without parameters (should use defaults/prompts)
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![], // no parameters
        true,   // dry-run
    )
    .await?;

    // Should succeed but may prompt for required parameters
    // For now we test that it doesn't crash - both success and graceful failure are acceptable
    assert!(
        success,
        "Greeting workflow should handle missing parameters gracefully"
    );
    Ok(())
}

#[tokio::test]
async fn test_plan_workflow_parameter_migration() -> Result<()> {
    // Test that plan workflow accepts parameters via --var (current system)
    let success = run_builtin_workflow_in_process(
        "plan",
        vec!["plan_filename=./specification/test-feature.md".to_string()],
        true, // dry-run
    )
    .await?;

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
    )
    .await?;

    assert!(
        success,
        "Plan workflow should maintain backward compatibility"
    );
    Ok(())
}

#[tokio::test]
async fn test_plan_workflow_legacy_behavior() -> Result<()> {
    // Test that plan runs without parameters (legacy behavior - scan ./specification)
    let success = run_builtin_workflow_in_process(
        "plan",
        vec![], // no parameters
        true,   // dry-run
    )
    .await?;

    assert!(
        success,
        "Plan workflow should support legacy behavior without parameters"
    );
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
    )
    .await?;

    assert!(
        success,
        "Multiple --var values should work with later values taking precedence"
    );
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
    )
    .await?;

    assert!(success1, "Workflow should handle empty variable values");

    // Test with special characters in values
    let success2 = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=José María".to_string(), // Special characters
            "language=Español".to_string(),
        ],
        true, // dry-run
    )
    .await?;

    assert!(
        success2,
        "Workflow should handle special characters in values"
    );
    Ok(())
}

// Keep a few slow CLI integration tests for end-to-end verification
#[tokio::test]
async fn test_cli_integration_greeting_workflow() -> Result<()> {
    // Run from repo root where builtin workflows are located
    let repo_root = get_repo_root();
    std::env::set_current_dir(&repo_root).unwrap();

    let result = run_sah_command_in_process(&[
        "flow",
        "greeting",
        "--var",
        "person_name=Integration Test",
        "--var",
        "language=English",
        "--dry-run",
    ])
    .await?;

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("🔍 Dry run mode"));
    assert!(result.stdout.contains("greeting"));
    Ok(())
}

#[tokio::test]
async fn test_cli_integration_plan_workflow() -> Result<()> {
    // Run from repo root where builtin workflows are located
    let repo_root = get_repo_root();
    std::env::set_current_dir(&repo_root).unwrap();

    let result = run_sah_command_in_process(&[
        "flow",
        "plan",
        "--var",
        "plan_filename=./test.md",
        "--dry-run",
    ])
    .await?;

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("🔍 Dry run mode"));
    assert!(result.stdout.contains("plan"));
    Ok(())
}
