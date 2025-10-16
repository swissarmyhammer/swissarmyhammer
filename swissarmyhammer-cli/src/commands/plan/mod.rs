//! Plan command implementation
//!
//! Executes planning workflow for specific specification files.
//!
//! # Design Note: Deprecation Warning Output
//!
//! Deprecation warnings use `tracing::warn!` instead of `eprintln!` because:
//! - Integrates with application logging infrastructure
//! - Automatically writes to stderr
//! - Can be controlled via log levels and filters
//! - Consistent with other user-facing messages in the codebase

use crate::cli::FlowSubcommand;
use crate::context::CliContext;

/// Help text for the plan command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Execute the plan workflow for a specific specification file
///
/// This command delegates to the flow runner to execute the 'plan' workflow,
/// which processes a specification file and generates an implementation plan.
///
/// **DEPRECATED**: This wrapper command is deprecated. Use `sah flow plan <file>` or
/// the dynamic shortcut `sah plan <file>` instead.
///
/// # Arguments
/// * `plan_filename` - Path to the specification file to process
/// * `context` - CLI context containing global arguments and configuration
///
/// # Returns
/// * `i32` - Exit code (0 for success, non-zero for error)
pub async fn handle_command(plan_filename: String, context: &CliContext) -> i32 {
    // Print deprecation warning
    if !context.quiet {
        tracing::warn!("'sah plan <file>' wrapper command is deprecated.");
        tracing::warn!(
            "  Use 'sah flow plan <file>' or 'sah plan <file>' (via dynamic shortcut) instead."
        );
        tracing::warn!("  This wrapper will be removed in a future version.");
        tracing::warn!("");
    }

    // Execute the plan workflow - equivalent to 'flow plan spec.md'
    // The plan_filename is now a positional argument
    let subcommand = FlowSubcommand::Execute {
        workflow: "plan".to_string(),
        positional_args: vec![plan_filename],
        params: vec![],
        vars: vec![],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
