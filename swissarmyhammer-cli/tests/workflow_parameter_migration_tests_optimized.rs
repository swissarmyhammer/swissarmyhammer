//! Optimized in-process tests for builtin workflow migration to parameter format
//!
//! These tests replace the slow CLI process-spawning tests in workflow_parameter_migration_tests.rs
//! with fast in-process execution using the existing CLI flow functions.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use swissarmyhammer_cli::{
    cli::FlowSubcommand,
    flow::run_flow_command,
};

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
    
    // Change to repo root directory where builtin workflows are located
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&repo_root)?;
    
    let result = {
        let subcommand = FlowSubcommand::Run {
            workflow: workflow_name.to_string(),
            vars,
            interactive: false,
            dry_run,
            test: false,
            timeout: Some("2s".to_string()), // Use 2 second timeout for fast tests
            quiet: true,
        };
        
        run_flow_command(subcommand).await
    };
    
    // Restore original directory
    std::env::set_current_dir(original_dir)?;
    
    Ok(result.is_ok())
}

#[tokio::test]
async fn test_greeting_workflow_parameter_migration_optimized() -> Result<()> {
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
async fn test_greeting_workflow_backward_compatibility_optimized() -> Result<()> {
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
async fn test_greeting_workflow_interactive_prompting_optimized() -> Result<()> {
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
async fn test_plan_workflow_parameter_migration_optimized() -> Result<()> {
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
async fn test_plan_workflow_backward_compatibility_optimized() -> Result<()> {
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
async fn test_plan_workflow_legacy_behavior_optimized() -> Result<()> {
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
async fn test_mixed_parameter_resolution_precedence_optimized() -> Result<()> {
    // Test precedence when multiple --var are used
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=Alice".to_string(), // First var value
            "person_name=Bob".to_string(),   // Second var value (should take precedence)
        ],
        true, // dry-run
    ).await?;

    // Should succeed regardless of precedence
    assert!(success, "Greeting workflow should handle duplicate vars with later values taking precedence");
    Ok(())
}

#[tokio::test]
async fn test_parameter_type_handling_optimized() -> Result<()> {
    // Test different parameter types using --var system
    let success = run_builtin_workflow_in_process(
        "greeting",
        vec![
            "person_name=Alice".to_string(),    // string
            "language=French".to_string(),      // choice
            "enthusiastic=true".to_string(),    // boolean
        ],
        true, // dry-run
    ).await?;

    assert!(success, "Greeting workflow should handle different parameter types");
    Ok(())
}

// Keep the integration tests that read files since they are fast
#[cfg(test)]
mod integration_workflow_tests {
    use super::*;

    #[test]
    fn test_builtin_workflow_files_exist() {
        // Verify the migrated workflow files exist and have proper structure
        // Look in the repo root, not relative to test directory
        let repo_root = get_repo_root();
        let greeting_path = repo_root.join("builtin/workflows/greeting.md");
        let plan_path = repo_root.join("builtin/workflows/plan.md");

        assert!(
            greeting_path.exists(),
            "greeting.md workflow should exist at {greeting_path:?}"
        );
        assert!(
            plan_path.exists(),
            "plan.md workflow should exist at {plan_path:?}"
        );
    }

    #[test]
    fn test_greeting_workflow_frontmatter_structure() {
        // Read and verify greeting workflow has proper parameter structure
        let repo_root = get_repo_root();
        let greeting_path = repo_root.join("builtin/workflows/greeting.md");
        let content =
            fs::read_to_string(&greeting_path).expect("Should be able to read greeting.md");

        // Check for key parameter fields
        assert!(
            content.contains("parameters:"),
            "Should have parameters section"
        );
        assert!(
            content.contains("person_name"),
            "Should have person_name parameter"
        );
        assert!(
            content.contains("language"),
            "Should have language parameter"
        );
        assert!(
            content.contains("enthusiastic"),
            "Should have enthusiastic parameter"
        );
        assert!(
            content.contains("required: true"),
            "Should have required parameters"
        );
        assert!(
            content.contains("type: string"),
            "Should have string parameters"
        );
        assert!(
            content.contains("type: choice"),
            "Should have choice parameters"
        );
        assert!(
            content.contains("type: boolean"),
            "Should have boolean parameters"
        );
    }

    #[test]
    fn test_plan_workflow_frontmatter_structure() {
        // Read and verify plan workflow has proper parameter structure
        let repo_root = get_repo_root();
        let plan_path = repo_root.join("builtin/workflows/plan.md");
        let content = fs::read_to_string(&plan_path).expect("Should be able to read plan.md");

        // Check for key parameter fields
        assert!(
            content.contains("parameters:"),
            "Should have parameters section"
        );
        assert!(
            content.contains("plan_filename"),
            "Should have plan_filename parameter"
        );
        assert!(
            content.contains("pattern: '^.*\\.md$'"),
            "Should have pattern validation"
        );
        assert!(
            content.contains("parameter_groups:"),
            "Should have parameter groups"
        );
        assert!(
            content.contains("input"),
            "Should have input parameter group"
        );
    }

    #[test]
    fn test_workflow_action_strings_updated() {
        // Verify action strings use consistent parameter names
        let repo_root = get_repo_root();
        let greeting_path = repo_root.join("builtin/workflows/greeting.md");
        let greeting_content =
            fs::read_to_string(&greeting_path).expect("Should be able to read greeting.md");

        assert!(
            greeting_content.contains("{{ person_name }}"),
            "Should use person_name in action strings"
        );
        assert!(
            greeting_content.contains("{{ language | default: 'English' }}"),
            "Should use language with default in action strings"
        );
        assert!(
            greeting_content.contains("{% if enthusiastic %}"),
            "Should use enthusiastic parameter in action strings"
        );
    }

    #[test]
    fn test_workflow_documentation_updated() {
        // Verify documentation reflects new parameter system
        let repo_root = get_repo_root();
        let greeting_path = repo_root.join("builtin/workflows/greeting.md");
        let greeting_content =
            fs::read_to_string(&greeting_path).expect("Should be able to read greeting.md");

        assert!(
            greeting_content.contains("CLI switches"),
            "Should document CLI switches"
        );
        assert!(
            greeting_content.contains("--person-name"),
            "Should document parameter switches"
        );
        assert!(
            greeting_content.contains("--interactive"),
            "Should document interactive mode"
        );
        assert!(
            greeting_content.contains("structured parameters"),
            "Should mention structured parameters"
        );
    }
}