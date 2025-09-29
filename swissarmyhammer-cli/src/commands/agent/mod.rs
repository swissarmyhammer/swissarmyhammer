//! Agent command implementation
//!
//! Manages and interacts with agents in the SwissArmyHammer system

pub mod display;
pub mod list;
pub mod use_command;

use crate::cli::AgentSubcommand;
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};

/// Help text for the agent command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the agent command - PURE ROUTING ONLY
pub async fn handle_command(subcommand: AgentSubcommand, context: &CliContext) -> i32 {
    let result = match subcommand {
        AgentSubcommand::List { format } => list::execute_list_command(format, context).await,
        AgentSubcommand::Use { agent_name } => {
            use_command::execute_use_command(agent_name, context).await
        }
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            // Don't double-print errors that are already formatted by subcommands
            let error_msg = e.to_string();
            if !error_msg.starts_with("❌") && !error_msg.contains("Failed to") {
                tracing::error!("Agent operation failed: {}", e);
                tracing::error!("Run 'sah agent list' to see available agents or 'sah agent --help' for usage.");
            }
            EXIT_ERROR
        }
    }
}

// NO business logic here - only routing and error handling
