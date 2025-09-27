//! Agent command implementation
//!
//! Manages and interacts with agents in the SwissArmyHammer system

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
            eprintln!("Agent command failed: {}", e);
            EXIT_ERROR
        }
    }
}

// NO business logic here - only routing and error handling