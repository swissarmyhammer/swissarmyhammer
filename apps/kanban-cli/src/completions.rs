//! Shell completion script generation for the `kanban` binary — thin shim
//! over [`swissarmyhammer_cli_completions::print_completion_for`]. Takes a
//! fully-assembled `Command` from `main.rs::build_cli` (the kanban tree is
//! schema-driven and built at runtime).

use clap::Command;
use clap_complete::Shell;
use std::io;

/// Write a shell completion script for `kanban` to stdout.
pub fn print_completion(cmd: Command, shell: Shell) -> io::Result<()> {
    swissarmyhammer_cli_completions::print_completion_for(cmd, "kanban", shell)
}
