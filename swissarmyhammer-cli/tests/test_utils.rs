//! Test utilities for SwissArmyHammer CLI tests
//!
//! This module extends the test utilities from the main crate with CLI-specific helpers.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Re-export the ProcessGuard from the main crate's test_utils
#[allow(unused_imports)]
pub use swissarmyhammer::test_utils::ProcessGuard;

// Re-export commonly used test utilities from the main crate
#[allow(unused_imports)]
pub use swissarmyhammer::test_utils::{
    create_semantic_test_guard, create_simple_test_prompt, create_test_home_guard,
    create_test_prompt_library, create_test_prompts, get_test_home, get_test_swissarmyhammer_dir,
    SemanticTestGuard, TestHomeGuard,
};

/// Create a temporary directory for testing
///
/// This is a convenience wrapper that provides consistent error handling.
/// Note: Consider using swissarmyhammer::test_utils::create_temp_dir() for new code.
#[allow(dead_code)]
pub fn create_temp_dir() -> Result<TempDir> {
    Ok(TempDir::new()?)
}

/// Create test prompt files in a directory
///
/// This creates actual prompt files on disk for integration testing.
/// Different from the main crate's create_test_prompts which creates Prompt objects.
#[allow(dead_code)]
pub fn create_test_prompt_files(prompts_dir: &Path) -> Result<()> {
    let test_prompts = vec![
        ("simple", "Hello, world!", vec![]),
        (
            "with_args",
            "Hello {{name}}, you are {{age}} years old",
            vec![("name", "User's name", true), ("age", "User's age", true)],
        ),
        (
            "code_review",
            "Review this code: {{ code }}",
            vec![("code", "Code to review", true)],
        ),
        (
            "bug_fix",
            "Fix this bug: {{ error }}",
            vec![("error", "Error message", true)],
        ),
        (
            "test_generation",
            "Generate tests for: {{ function }}",
            vec![("function", "Function to test", true)],
        ),
    ];

    for (name, template, args) in test_prompts {
        let prompt_file = prompts_dir.join(format!("{name}.prompt"));
        let mut yaml_content = String::from("---\n");
        yaml_content.push_str(&format!("name: {name}\n"));
        yaml_content.push_str(&format!("description: Test prompt for {name}\n"));

        if !args.is_empty() {
            yaml_content.push_str("arguments:\n");
            for (arg_name, desc, required) in args {
                yaml_content.push_str(&format!("  - name: {arg_name}\n"));
                yaml_content.push_str(&format!("    description: {desc}\n"));
                yaml_content.push_str(&format!("    required: {required}\n"));
            }
        }

        yaml_content.push_str("---\n");
        yaml_content.push_str(template);

        std::fs::write(&prompt_file, yaml_content)?;
    }

    Ok(())
}

/// Create a temporary test environment with prompts
///
/// Returns a TempDir and the path to the prompts directory
#[allow(dead_code)]
pub fn create_test_environment() -> Result<(TempDir, PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    let prompts_dir = swissarmyhammer_dir.join("prompts");

    std::fs::create_dir_all(&prompts_dir)?;
    create_test_prompt_files(&prompts_dir)?;

    Ok((temp_dir, prompts_dir))
}

/// Setup environment for MCP tests
///
/// Sets HOME to a temporary directory and creates the necessary structure
#[allow(dead_code)]
pub fn setup_mcp_test_env() -> Result<(TempDir, PathBuf)> {
    let temp_dir = create_temp_dir()?;
    std::env::set_var("HOME", temp_dir.path());

    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    let prompts_dir = swissarmyhammer_dir.join("prompts");

    std::fs::create_dir_all(&prompts_dir)?;
    create_test_prompt_files(&prompts_dir)?;

    Ok((temp_dir, prompts_dir))
}

// SemanticTestGuard is now available from the main crate via the re-export above
