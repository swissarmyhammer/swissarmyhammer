//! Validate command implementation
//!
//! Validates prompt files and workflows for syntax and best practices

use crate::cli::OutputFormat;
use crate::exit_codes::EXIT_ERROR;
use crate::validate;

/// Help text for the validate command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the validate command
pub async fn handle_command(
    quiet: bool,
    format: OutputFormat,
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> i32 {
    match validate::run_validate_command_with_dirs(quiet, format, workflow_dirs, validate_tools)
        .await
    {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Validate command failed: {}", e);
            EXIT_ERROR
        }
    }
}
