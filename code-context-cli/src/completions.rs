//! Shell completion script generation for the `code-context` binary —
//! thin shim over [`swissarmyhammer_cli_completions::print_completion`].

use crate::cli::Cli;
use clap_complete::Shell;
use std::io;

/// Write a shell completion script for `code-context` to stdout.
pub fn print_completion(shell: Shell) -> io::Result<()> {
    swissarmyhammer_cli_completions::print_completion::<Cli>("code-context", shell)
}
