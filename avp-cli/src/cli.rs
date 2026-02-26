//! CLI definition for the AVP command-line interface.
//!
//! This module is self-contained — it only depends on `clap` and `std` so that
//! `build.rs` can compile it independently via `#[path = "src/cli.rs"]` to
//! generate documentation, man pages, and shell completions at build time.

use clap::{Parser, Subcommand, ValueEnum};

/// Target location for install/uninstall operations.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InstallTarget {
    /// Project-level settings (.claude/settings.json)
    Project,
    /// Local project settings, not committed (.claude/settings.local.json)
    Local,
    /// User-level settings (~/.claude/settings.json)
    User,
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallTarget::Project => write!(f, "project"),
            InstallTarget::Local => write!(f, "local"),
            InstallTarget::User => write!(f, "user"),
        }
    }
}

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
}
