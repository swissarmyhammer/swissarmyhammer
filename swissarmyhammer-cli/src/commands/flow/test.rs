//! Test a workflow without executing actions command implementation

use super::run::{execute_run_command, RunCommandConfig};
use crate::context::CliContext;
use swissarmyhammer::Result;

/// Execute the test workflow command
pub async fn execute_test_command(
    workflow: String,
    vars: Vec<String>,
    interactive: bool,
    quiet: bool,
    context: &CliContext,
) -> Result<()> {
    // Test workflow is the same as run workflow but with dry_run enabled
    execute_run_command(
        RunCommandConfig {
            workflow,
            positional_args: vec![], // No positional args for test
            params: vec![],          // No --param args for test
            vars,
            interactive,
            dry_run: true, // dry_run = true for test mode
            quiet,
        },
        context,
    )
    .await
}
