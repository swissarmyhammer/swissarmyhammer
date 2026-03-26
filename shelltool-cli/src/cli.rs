//! CLI definition for the shelltool command-line interface.
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

/// shelltool - A shell that works the way you do
///
/// Agents shouldn't drown in raw command output. shelltool gives them a
/// persistent shell with searchable history — run commands, then grep or
/// semantic-search the results instead of clogging the context window.
#[derive(Parser, Debug)]
#[command(name = "shelltool")]
#[command(version)]
#[command(about = "A shell that saves tokens — run, search, retrieve")]
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
}
