//! Implement command implementation
//!
//! Executes the implement workflow for autonomous issue resolution

use crate::cli::FlowSubcommand;

/// Help text for the implement command
pub const DESCRIPTION: &str = include_str!("description.md");

use crate::context::CliContext;

/// Execute the implement workflow for autonomous issue resolution
///
/// This command delegates to the flow runner to execute the 'implement' workflow,
/// which automatically resolves pending issues in the repository.
///
/// # Arguments
/// * `context` - CLI context containing global arguments and configuration
///
/// # Returns
/// * `i32` - Exit code (0 for success, non-zero for error)
pub async fn handle_command(context: &CliContext) -> i32 {
    // Execute the implement workflow - equivalent to 'flow implement'
    let subcommand = FlowSubcommand::Execute {
        workflow: "implement".to_string(),
        positional_args: vec![],
        params: vec![],
        vars: vec![],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
