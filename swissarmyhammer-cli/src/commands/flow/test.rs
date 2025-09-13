//! Test a workflow without executing actions command implementation

use super::run::execute_run_command;
use crate::context::CliContext;
use swissarmyhammer::Result;

/// Execute the test workflow command
pub async fn execute_test_command(
    workflow: String,
    vars: Vec<String>,
    interactive: bool,
    timeout: Option<String>,
    quiet: bool,
    context: &CliContext,
) -> Result<()> {
    // Test workflow is the same as run workflow but with dry_run enabled
    execute_run_command(
        workflow,
        vars,
        interactive,
        true, // dry_run = true for test mode
        timeout,
        quiet,
        context,
    )
    .await
}
