//! Implement command implementation
//! 
//! Executes the implement workflow for autonomous issue resolution

use crate::cli::FlowSubcommand;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use crate::flow;

/// Help text for the implement command
pub const DESCRIPTION: &str = include_str!("description.md");



/// Handle the implement command
pub async fn handle_command() -> i32 {
    // Execute the implement workflow - equivalent to 'flow run implement'
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: vec![],
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };

    match flow::run_flow_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Implement command failed: {}", e);
            EXIT_ERROR
        }
    }
}