//! CLI definition for the shelltool command-line interface.
//!
//! `build.rs` compiles this module independently via `#[path = "src/cli.rs"]`
//! to generate documentation, man pages, and shell completions at build time.
//! Beyond `clap` and `std`, it depends only on the shared
//! [`swissarmyhammer_cli_completions::lifecycle::InstallTarget`] enum, which is
//! declared as a build dependency of this crate — so `build.rs`'s `#[path]`
//! compilation has it available. `InstallTarget` is the single canonical
//! install-scope type shared by every tool CLI, rather than a per-binary copy.

use clap::{Parser, Subcommand};
use swissarmyhammer_cli_completions::lifecycle::InstallTarget;

/// shelltool - A shell that saves tokens
///
/// Replaces Bash and exec CLI tools with a persistent, searchable shell.
/// Instead of flooding the context window with raw command output, shelltool
/// stores everything in history — the agent runs commands, then greps the
/// results, retrieving only the lines that matter.
#[derive(Parser, Debug)]
#[command(name = "shelltool")]
#[command(version)]
#[command(about = "Replaces Bash/exec with a searchable shell that saves tokens")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// shelltool subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run MCP server over stdio, exposing the shell tool
    Serve,
    /// Install shelltool MCP server into Claude Code settings
    Init {
        /// Where to install the server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove shelltool from Claude Code settings
    Deinit {
        /// Where to remove the server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose shelltool configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
    /// Generate shell completion scripts
    #[command(long_about = "
Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  shelltool completion bash > ~/.local/share/bash-completion/completions/shelltool

  # Zsh (add to ~/.zshrc or a file in fpath)
  shelltool completion zsh > ~/.zfunc/_shelltool

  # Fish
  shelltool completion fish > ~/.config/fish/completions/shelltool.fish

  # PowerShell
  shelltool completion powershell >> $PROFILE
")]
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
