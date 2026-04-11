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

/// shelltool - A shell that saves tokens
///
/// Replaces Bash and exec CLI tools with a persistent, searchable shell.
/// Instead of flooding the context window with raw command output, shelltool
/// stores everything in history — the agent runs commands, then greps or
/// semantic-searches the results, retrieving only the lines that matter.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that each `InstallTarget` variant renders to the expected
    /// lowercase string via its `Display` impl. The rendered values are part
    /// of the CLI's user-facing contract (they appear in help text, default
    /// value display, and error messages), so locking them in with a test
    /// guards against accidental casing or spelling changes.
    #[test]
    fn install_target_display_renders_each_variant() {
        assert_eq!(InstallTarget::Project.to_string(), "project");
        assert_eq!(InstallTarget::Local.to_string(), "local");
        assert_eq!(InstallTarget::User.to_string(), "user");

        // Exercise the `format!` path explicitly to ensure the `Formatter`
        // branch of the impl is covered, not just `ToString::to_string`.
        assert_eq!(format!("{}", InstallTarget::Project), "project");
    }
}
