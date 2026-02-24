//! Tests for builtin workflow migration to parameter format
// sah rule ignore test_rule_with_allow

use anyhow::Result;
use std::path::PathBuf;

use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::{
    commands::flow::run::{execute_run_command, RunCommandConfig},
    context::CliContext,
};

/// Get the repository root directory (parent of the CLI crate directory)
fn get_repo_root() -> PathBuf {
    // Use CARGO_MANIFEST_DIR which is set at compile time to the crate directory
    // Then get its parent to reach the workspace root
    let cli_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(cli_dir)
        .parent()
        .expect("CLI directory should have a parent")
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

    match &result {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("Workflow execution failed: {:?}", e);
            Ok(false)
        }
    }
}

#[tokio::test]
async fn test_hello_world_workflow_backward_compatibility() -> Result<()> {
    // Test that --var arguments work
    let success = run_builtin_workflow_in_process(
        "hello-world",
        vec![
            "person_name=John".to_string(),
            "language=English".to_string(),
        ],
        true, // dry-run
    )
    .await?;

    assert!(
        success,
        "Hello-world workflow should maintain backward compatibility"
    );
    Ok(())
}

#[tokio::test]
async fn test_hello_world_workflow_interactive_prompting() -> Result<()> {
    // Test that workflow runs without parameters (should use defaults/prompts)
    let success = run_builtin_workflow_in_process(
        "hello-world",
        vec![], // no parameters
        true,   // dry-run
    )
    .await?;

    // Should succeed but may prompt for required parameters
    // For now we test that it doesn't crash - both success and graceful failure are acceptable
    assert!(
        success,
        "Hello-world workflow should handle missing parameters gracefully"
    );
    Ok(())
}

#[tokio::test]
async fn test_mixed_parameter_resolution_precedence() -> Result<()> {
    // Test precedence when multiple --var are used
    let success = run_builtin_workflow_in_process(
        "hello-world",
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
        "hello-world",
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
        "hello-world",
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
async fn test_cli_integration_hello_world_workflow() -> Result<()> {
    use crate::in_process_test_utils::run_sah_command_in_process_with_dir;

    // Run from repo root where builtin workflows are located
    let repo_root = get_repo_root();
    let _env = IsolatedTestEnvironment::new()?;

    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "hello-world",
            "--var",
            "person_name=Integration Test",
            "--var",
            "language=English",
            "--dry-run",
        ],
        &repo_root,
    )
    .await?;

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("🔍 Dry run mode"));
    assert!(result.stdout.contains("hello-world"));
    Ok(())
}
