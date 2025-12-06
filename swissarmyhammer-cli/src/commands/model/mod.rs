//! Agent command implementation
//!
//! Manages and interacts with agents in the SwissArmyHammer system
//! sah rule ignore test_rule_with_allow

pub mod display;
pub mod list;
pub mod show;
pub mod use_command;

use crate::cli::ModelSubcommand;
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};

/// Help text for the agent command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the agent command - PURE ROUTING ONLY
pub async fn handle_command(subcommand: Option<ModelSubcommand>, context: &CliContext) -> i32 {
    let result = match subcommand {
        Some(ModelSubcommand::List { format }) => list::execute_list_command(format, context).await,
        Some(ModelSubcommand::Show { .. }) | None => show::execute_show_command(context).await,
        Some(ModelSubcommand::Use { first, second }) => {
            use_command::execute_use_command(first, second, context).await
        }
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            // Don't double-print errors that are already formatted by subcommands
            let error_msg = e.to_string();
            if !error_msg.starts_with("✗") && !error_msg.contains("Failed to") {
                tracing::error!("Model operation failed: {}", e);
                tracing::error!(
                    "Run 'sah model list' to see available models or 'sah model --help' for usage."
                );
            }
            EXIT_ERROR
        }
    }
}

// NO business logic here - only routing and error handling
