//! CLI definition for the code-context command-line interface.
//!
//! `build.rs` compiles this module independently via `#[path = "src/cli.rs"]`
//! to generate documentation, man pages, and shell completions at build time.
//! Beyond `clap` and `std`, it depends only on the shared
//! [`swissarmyhammer_cli_completions::lifecycle::InstallTarget`] enum, which is
//! declared as a build dependency of this crate -- so `build.rs`'s `#[path]`
//! compilation has it available. `InstallTarget` is the single canonical
//! install-scope type shared by every tool CLI, rather than a per-binary copy.
//!
//! The schema-driven operation commands (`get symbol`, `search code`, etc.) are
//! NOT defined here -- they are built dynamically in `main.rs` from the
//! `CodeContextTool` full schema via `swissarmyhammer_operations::cli_gen`. This
//! file only defines the lifecycle commands: serve, init, deinit, doctor, skill,
//! and completion.

use clap::{Parser, Subcommand};
use swissarmyhammer_cli_completions::lifecycle::InstallTarget;

/// code-context - Structural code intelligence for AI agents
///
/// Provides indexed code navigation, symbol lookup, call graph traversal,
/// blast radius analysis, and semantic search. Exposes these capabilities
/// as MCP tools for AI coding agents.
#[derive(Parser, Debug)]
#[command(name = "code-context")]
#[command(version)]
#[command(about = "Structural code intelligence for AI coding agents")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Output results as JSON (for operation commands)
    #[arg(short, long, global = true)]
    pub json: bool,

    /// Disable interactive progress bars for long-running operations.
    ///
    /// `indicatif` auto-degrades to plain output on non-TTY stdout, but
    /// some environments (CI runners, recording wrappers) still benefit
    /// from a hard switch. With this flag set the dispatcher installs
    /// a no-op renderer and the tool emits no progress chrome.
    #[arg(long, global = true)]
    pub no_progress: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// code-context lifecycle subcommands.
///
/// Operation commands (verb-noun pattern, e.g. `get symbol`) are not declared
/// here; they are generated at runtime from the `CodeContextTool` schema in
/// `main.rs`. Only the static lifecycle commands live here so `build.rs` can
/// compile this module standalone for doc/manpage/completion generation.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run MCP server over stdio, exposing code-context tools
    Serve,
    /// Install code-context MCP server into Claude Code settings
    Init {
        /// Where to install the server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove code-context from Claude Code settings
    Deinit {
        /// Where to remove the server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose code-context configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
    /// Deploy code-context skill to agent .skills/ directories
    Skill,

    /// Generate shell completion scripts
    #[command(long_about = "
Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  code-context completion bash > ~/.local/share/bash-completion/completions/code-context

  # Zsh (add to ~/.zshrc or a file in fpath)
  code-context completion zsh > ~/.zfunc/_code-context

  # Fish
  code-context completion fish > ~/.config/fish/completions/code-context.fish

  # PowerShell
  code-context completion powershell >> $PROFILE
")]
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Cli {
    /// Parse CLI arguments, returning an error on failure instead of exiting.
    ///
    /// This is useful for testing and for `build.rs` which needs to introspect
    /// the command tree without actually running anything.
    #[allow(dead_code)]
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse args and return the `Cli`, panicking on failure.
    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["code-context"];
        full.extend_from_slice(args);
        Cli::try_parse_from_args(full).unwrap()
    }

    // -- Top-level help / version --

    #[test]
    fn help_displays_all_lifecycle_commands() {
        let err = Cli::try_parse_from_args(["code-context", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        for cmd in ["serve", "init", "deinit", "doctor", "skill"] {
            assert!(help.contains(cmd), "help missing command: {cmd}");
        }
    }

    #[test]
    fn version_flag() {
        let err = Cli::try_parse_from_args(["code-context", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    // -- Global flags --

    #[test]
    fn global_debug_flag() {
        let cli = parse(&["--debug", "serve"]);
        assert!(cli.debug);
    }

    #[test]
    fn global_json_flag() {
        let cli = parse(&["--json", "serve"]);
        assert!(cli.json);
    }

    // -- Lifecycle commands --

    #[test]
    fn serve_command() {
        let cli = parse(&["serve"]);
        assert!(matches!(cli.command, Commands::Serve));
    }

    #[test]
    fn init_defaults_to_project() {
        let cli = parse(&["init"]);
        assert!(matches!(
            cli.command,
            Commands::Init {
                target: InstallTarget::Project
            }
        ));
    }

    #[test]
    fn init_user() {
        let cli = parse(&["init", "user"]);
        assert!(matches!(
            cli.command,
            Commands::Init {
                target: InstallTarget::User
            }
        ));
    }

    #[test]
    fn deinit_defaults_to_project() {
        let cli = parse(&["deinit"]);
        assert!(matches!(
            cli.command,
            Commands::Deinit {
                target: InstallTarget::Project
            }
        ));
    }

    #[test]
    fn doctor_verbose() {
        let cli = parse(&["doctor", "--verbose"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: true }));
    }

    #[test]
    fn skill_command() {
        let cli = parse(&["skill"]);
        assert!(matches!(cli.command, Commands::Skill));
    }
}
