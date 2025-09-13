//! Plan command implementation
//!
//! Executes planning workflow for specific specification files

use crate::cli::FlowSubcommand;
use crate::context::CliContext;

/// Help text for the plan command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Execute the plan workflow for a specific specification file
///
/// This command delegates to the flow runner to execute the 'plan' workflow,
/// which processes a specification file and generates an implementation plan.
///
/// # Arguments
/// * `plan_filename` - Path to the specification file to process
/// * `context` - CLI context containing global arguments and configuration
///
/// # Returns
/// * `i32` - Exit code (0 for success, non-zero for error)
pub async fn handle_command(plan_filename: String, context: &CliContext) -> i32 {
    // Execute the plan workflow - equivalent to 'flow run plan'
    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![format!("plan_filename={}", plan_filename)],
        interactive: false,
        dry_run: false,
        timeout: None,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
