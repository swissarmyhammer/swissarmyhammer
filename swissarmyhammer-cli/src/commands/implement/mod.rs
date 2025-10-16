//! Implement command implementation
//!
//! Executes the implement workflow for autonomous issue resolution.
//!
//! # Design Note: Deprecation Warning Output
//!
//! Deprecation warnings use `tracing::warn!` instead of `eprintln!` because:
//! - Integrates with application logging infrastructure
//! - Automatically writes to stderr
//! - Can be controlled via log levels and filters
//! - Consistent with other user-facing messages in the codebase

use crate::cli::FlowSubcommand;

/// Help text for the implement command
pub const DESCRIPTION: &str = include_str!("description.md");

use crate::context::CliContext;

/// Execute the implement workflow for autonomous issue resolution
///
/// This command delegates to the flow runner to execute the 'implement' workflow,
/// which automatically resolves pending issues in the repository.
///
/// **DEPRECATED**: This wrapper command is deprecated. Use `sah flow implement` or
/// the dynamic shortcut `sah implement` instead.
///
/// # Arguments
/// * `context` - CLI context containing global arguments and configuration
///
/// # Returns
/// * `i32` - Exit code (0 for success, non-zero for error)
pub async fn handle_command(context: &CliContext) -> i32 {
    // Print deprecation warning
    if !context.quiet {
        tracing::warn!("'sah implement' wrapper command is deprecated.");
        tracing::warn!(
            "  Use 'sah flow implement' or 'sah implement' (via dynamic shortcut) instead."
        );
        tracing::warn!("  This wrapper will be removed in a future version.");
        tracing::warn!("");
    }

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
