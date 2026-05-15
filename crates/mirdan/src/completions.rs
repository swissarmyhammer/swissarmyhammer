//! Shell completion script generation for the `mirdan` binary — thin shim
//! over [`swissarmyhammer_cli_completions::print_completion`].

use crate::Cli;
use clap_complete::Shell;
use std::io;

/// Write a shell completion script for `mirdan` to stdout.
pub fn print_completion(shell: Shell) -> io::Result<()> {
    swissarmyhammer_cli_completions::print_completion::<Cli>("mirdan", shell)
}
