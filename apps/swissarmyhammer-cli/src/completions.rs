//! Shell completion script generation for the `sah` binary — thin shim
//! over [`swissarmyhammer_cli_completions::print_completion_for`]. Takes a
//! fully-assembled `Command` from `dynamic_cli::CliBuilder::build_cli`
//! (the sah tree is built at runtime from MCP tool registrations).

use anyhow::Result;
use clap::Command;
use clap_complete::Shell;

/// Write a shell completion script for `sah` to stdout.
pub fn print_completion_for(cmd: Command, shell: Shell) -> Result<()> {
    swissarmyhammer_cli_completions::print_completion_for(cmd, "sah", shell)?;
    Ok(())
}
