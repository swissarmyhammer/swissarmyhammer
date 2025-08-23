//! Prompt command implementation
//! 
//! Manages and tests prompts with support for listing, validating, testing, and searching

use crate::cli::PromptSubcommand;
use crate::exit_codes::EXIT_SUCCESS;
use crate::prompt;

/// Help text for the prompt command
pub const DESCRIPTION: &str = include_str!("description.md");



/// Handle the prompt command
pub async fn handle_command(subcommand: PromptSubcommand) -> i32 {
    match prompt::run_prompt_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Prompt command failed: {}", e);
            e.exit_code
        }
    }
}