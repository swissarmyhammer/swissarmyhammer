//! CLI definition for the AVP command-line interface.
//!
//! `build.rs` compiles this module independently via `#[path = "src/cli.rs"]`
//! to generate documentation, man pages, and shell completions at build time.
//! Beyond `clap` and `std`, it depends only on the shared
//! [`swissarmyhammer_cli_completions::lifecycle::InstallTarget`] enum, which is
//! declared as a build dependency of this crate — so `build.rs`'s `#[path]`
//! compilation has it available. `InstallTarget` is the single canonical
//! install-scope type shared by every tool CLI, rather than a per-binary copy.

use clap::{Parser, Subcommand};
pub use swissarmyhammer_cli_completions::lifecycle::InstallTarget;

/// AVP - Agent Validator Protocol
///
/// Claude Code hook processor that validates tool calls, file changes, and more.
#[derive(Parser, Debug)]
#[command(name = "avp")]
#[command(version)]
#[command(about = "Agent Validator Protocol - Claude Code hook processor")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install AVP hooks into Claude Code settings
    Init {
        /// Where to install the hooks
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove AVP hooks from Claude Code settings and delete .avp directory
    Deinit {
        /// Where to remove the hooks from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose AVP configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
    /// Edit an existing RuleSet in $EDITOR
    Edit {
        /// RuleSet name (kebab-case)
        name: String,
        /// Edit in project (.avp/validators/) [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Edit in user-level directory (~/.avp/validators/)
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
    /// Create a new RuleSet from template
    New {
        /// RuleSet name (kebab-case)
        name: String,
        /// Create in project (.avp/validators/) [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Create in user-level directory (~/.avp/validators/)
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
    /// Manage AI model configurations
    Model {
        #[command(subcommand)]
        action: Option<ModelAction>,
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
  avp completion bash > ~/.local/share/bash-completion/completions/avp

  # Zsh (add to ~/.zshrc or a file in fpath)
  avp completion zsh > ~/.zfunc/_avp

  # Fish
  avp completion fish > ~/.config/fish/completions/avp.fish

  # PowerShell
  avp completion powershell >> $PROFILE
")]
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// Model subcommands
#[derive(Subcommand, Debug)]
pub enum ModelAction {
    /// List all available models
    List,
    /// Show the current model configuration
    Show,
    /// Apply a specific model to the project
    Use {
        /// Model name to apply
        name: String,
    },
}
