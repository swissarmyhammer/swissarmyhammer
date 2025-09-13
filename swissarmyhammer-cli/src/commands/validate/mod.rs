//! Validate command implementation
//!
//! Validates prompt files and workflows for syntax and best practices

use crate::context::CliContext;
use crate::exit_codes::EXIT_ERROR;
use crate::validate;
use anyhow::Result;

pub mod display;

/// Help text for the validate command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the validate command using CliContext pattern
pub async fn handle_command(
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    cli_context: &CliContext,
) -> i32 {
    match run_validate_with_context(workflow_dirs, validate_tools, cli_context).await {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Validate command failed: {}", e);
            EXIT_ERROR
        }
    }
}

/// Run validation and display results using CliContext
async fn run_validate_with_context(
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    cli_context: &CliContext,
) -> Result<i32> {
    // Run validation to get structured results
    let (validation_result, exit_code) =
        validate::run_validate_command_structured(cli_context.quiet, workflow_dirs, validate_tools)
            .await?;

    // Convert to display objects and output using CliContext
    if cli_context.verbose {
        let verbose_results: Vec<display::VerboseValidationResult> = validation_result
            .issues
            .iter()
            .map(display::VerboseValidationResult::from)
            .collect();
        cli_context.display(verbose_results)?;
    } else {
        let results: Vec<display::ValidationResult> = validation_result
            .issues
            .iter()
            .map(display::ValidationResult::from)
            .collect();
        cli_context.display(results)?;
    }

    Ok(exit_code)
}
