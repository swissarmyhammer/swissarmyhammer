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

    // Convert to display objects and output using comfy-table with colored status
    use comfy_table::{Cell, Color};
    use swissarmyhammer::validation::ValidationLevel;

    let mut table = swissarmyhammer_doctor::new_table();

    if cli_context.verbose {
        table.set_header(vec!["Status", "File", "Result", "Fix", "File Type"]);
        for issue in &validation_result.issues {
            let result = display::VerboseValidationResult::from(issue);
            let status_cell = match issue.level {
                ValidationLevel::Info => Cell::new("✓").fg(Color::Green),
                ValidationLevel::Warning => Cell::new("⚠").fg(Color::Yellow),
                ValidationLevel::Error => Cell::new("✗").fg(Color::Red),
            };
            table.add_row(vec![
                status_cell,
                Cell::new(&result.file),
                Cell::new(&result.result),
                Cell::new(&result.fix),
                Cell::new(&result.file_type),
            ]);
        }
    } else {
        table.set_header(vec!["Status", "File", "Result"]);
        for issue in &validation_result.issues {
            let result = display::ValidationResult::from(issue);
            let status_cell = match issue.level {
                ValidationLevel::Info => Cell::new("✓").fg(Color::Green),
                ValidationLevel::Warning => Cell::new("⚠").fg(Color::Yellow),
                ValidationLevel::Error => Cell::new("✗").fg(Color::Red),
            };
            table.add_row(vec![
                status_cell,
                Cell::new(&result.file),
                Cell::new(&result.result),
            ]);
        }
    }

    println!("{table}");

    Ok(exit_code)
}
