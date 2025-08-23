//! Validate command implementation
//!
//! Validates prompt files and workflows for syntax and best practices

use crate::cli::ValidateFormat;
use crate::exit_codes::EXIT_ERROR;
use crate::validate;

/// Help text for the validate command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the validate command
pub fn handle_command(quiet: bool, format: ValidateFormat, workflow_dirs: Vec<String>) -> i32 {
    match validate::run_validate_command_with_dirs(quiet, format, workflow_dirs) {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Validate command failed: {}", e);
            EXIT_ERROR
        }
    }
}
