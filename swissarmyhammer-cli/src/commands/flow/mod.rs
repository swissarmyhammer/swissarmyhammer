//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

use crate::cli::FlowSubcommand;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use crate::flow;

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the flow command
pub async fn handle_command(subcommand: FlowSubcommand) -> i32 {
    match flow::run_flow_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Flow command failed: {}", e);
            EXIT_ERROR
        }
    }
}
