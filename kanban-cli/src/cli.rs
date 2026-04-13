//! CLI definition for the kanban command-line interface.
//!
//! This module is self-contained -- it only depends on `clap` and `std` so that
//! `build.rs` can compile it independently via `#[path = "src/cli.rs"]` to
//! generate documentation, man pages, and shell completions at build time.
//!
//! The schema-driven noun/verb commands (`task add`, `board init`, etc.) are NOT
//! defined here -- they are built dynamically in `main.rs` via `cli_gen`. This
//! file only defines the four lifecycle commands: serve, init, deinit, doctor.

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

/// kanban - Task management for AI coding agents
///
/// Standalone CLI for SwissArmyHammer Kanban board. Exposes board, task,
/// column, tag, and project operations as direct subcommands, and can run
/// as an MCP server for integration with Claude Code and other agents.
#[derive(Parser, Debug)]
#[command(name = "kanban")]
#[command(version)]
#[command(about = "Kanban board CLI — manage tasks, boards, and columns")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// kanban subcommands (lifecycle only -- noun/verb commands are built dynamically).
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run MCP server over stdio, exposing kanban tools
    Serve,
    /// Install kanban MCP server into Claude Code settings
    Init {
        /// Where to install the server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove kanban from Claude Code settings
    Deinit {
        /// Where to remove the server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose kanban configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
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
        let mut full = vec!["kanban"];
        full.extend_from_slice(args);
        Cli::try_parse_from_args(full).unwrap()
    }

    // -- Top-level help / version --

    #[test]
    fn help_displays_all_lifecycle_commands() {
        let err = Cli::try_parse_from_args(["kanban", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        for cmd in ["serve", "init", "deinit", "doctor"] {
            assert!(help.contains(cmd), "help missing command: {cmd}");
        }
    }

    #[test]
    fn version_flag() {
        let err = Cli::try_parse_from_args(["kanban", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    // -- Global flags --

    #[test]
    fn global_debug_flag() {
        let cli = parse(&["--debug", "serve"]);
        assert!(cli.debug);
    }

    // -- Serve --

    #[test]
    fn serve_command() {
        let cli = parse(&["serve"]);
        assert!(matches!(cli.command, Commands::Serve));
    }

    // -- Init --

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
    fn init_local() {
        let cli = parse(&["init", "local"]);
        assert!(matches!(
            cli.command,
            Commands::Init {
                target: InstallTarget::Local
            }
        ));
    }

    // -- Deinit --

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
    fn deinit_user() {
        let cli = parse(&["deinit", "user"]);
        assert!(matches!(
            cli.command,
            Commands::Deinit {
                target: InstallTarget::User
            }
        ));
    }

    #[test]
    fn deinit_local() {
        let cli = parse(&["deinit", "local"]);
        assert!(matches!(
            cli.command,
            Commands::Deinit {
                target: InstallTarget::Local
            }
        ));
    }

    // -- Doctor --

    #[test]
    fn doctor_no_verbose() {
        let cli = parse(&["doctor"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: false }));
    }

    #[test]
    fn doctor_verbose() {
        let cli = parse(&["doctor", "--verbose"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: true }));
    }

    #[test]
    fn doctor_verbose_short() {
        let cli = parse(&["doctor", "-v"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: true }));
    }

    // -- InstallTarget Display --

    #[test]
    fn install_target_display() {
        assert_eq!(InstallTarget::Project.to_string(), "project");
        assert_eq!(InstallTarget::Local.to_string(), "local");
        assert_eq!(InstallTarget::User.to_string(), "user");
    }
}
