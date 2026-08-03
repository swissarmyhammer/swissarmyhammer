//! CLI definition for the swissarmyhammer (`sah`) command-line interface.
//!
//! `build.rs` compiles this module independently via `#[path = "src/cli.rs"]`
//! to generate documentation, man pages, and shell completions at build time.
//! Beyond `clap` and `std`, it depends only on the shared
//! [`swissarmyhammer_cli_completions::lifecycle::InstallTarget`] enum (re-exported
//! below), which is declared as a build dependency of this crate — so `build.rs`'s
//! `#[path]` compilation has it available. `InstallTarget` is the single canonical
//! install-scope type shared by every tool CLI, and its
//! `From<InstallTarget> for InitScope` lives with it in that shared crate.
//!
//! Cross-crate type conversions (e.g. `SourceArg <-> FileSource`,
//! `InstallTarget -> InitScope`) live in `crate::cli_conversions` so that
//! `cli.rs` does not pull in library dependencies.

use clap::{Parser, Subcommand, ValueEnum};
use std::str::FromStr;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            "table" => Ok(OutputFormat::Table),
            _ => Ok(OutputFormat::Table), // Default to Table for unknown formats
        }
    }
}

/// CLI wrapper for the library's `FileSource` enum (which does not derive
/// `ValueEnum`). Conversions to/from `FileSource` live in
/// `crate::cli_conversions`.
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum SourceArg {
    Builtin,
    User,
    Local,
    Dynamic,
}

/// Target location for init/deinit operations.
///
/// Re-exported from the canonical shared [`InstallTarget`] so there is exactly
/// one such enum (and one `From<InstallTarget> for InitScope`) across every
/// workspace CLI.
pub use swissarmyhammer_cli_completions::lifecycle::InstallTarget;

#[derive(Parser, Debug)]
#[command(name = "swissarmyhammer")]
#[command(version)]
#[command(about = "An MCP server that brings skills, workflows, and agents to AI coding tools")]
#[command(long_about = "
swissarmyhammer is an MCP (Model Context Protocol) server that brings skills,
workflows, and agents to AI coding tools. It supports template substitution
and seamless integration with Claude Code and other ACP-compatible editors.

Global arguments can be used with any command to control output and behavior:
  --verbose     Show detailed information and debug output
  --format      Set output format (table, json, yaml) for commands that support it
  --debug       Enable debug mode with comprehensive tracing
  --quiet       Suppress all output except errors

Main commands:
  serve         Run as MCP server (default when invoked via stdio)
  init          Set up sah for all detected AI coding agents (skills + MCP)
  doctor        Diagnose configuration and setup issues
  validate      Validate configuration files for syntax and best practices
  completion    Generate shell completion scripts

Example usage:
  swissarmyhammer serve                           # Run as MCP server
  swissarmyhammer init                            # Set up skills + MCP for detected agents
  swissarmyhammer doctor                          # Check configuration
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug logging
    #[arg(short, long)]
    pub debug: bool,

    /// Suppress all output except errors
    #[arg(short, long)]
    pub quiet: bool,

    /// Global output format
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run as MCP server (default when invoked via stdio)
    #[command(long_about = "
Run as MCP server. This is the default mode when
invoked via stdio (e.g., by Claude Code). The server will:

- Expose the SwissArmyHammer tools and workflows via the MCP protocol
- Watch for file changes and reload automatically

Example:
  swissarmyhammer serve        # Stdio mode (default)
  swissarmyhammer serve http   # HTTP mode
  # Or configure in Claude Code's MCP settings
")]
    Serve {
        #[command(subcommand)]
        subcommand: Option<ServeSubcommand>,
    },
    /// Set up sah for all detected AI coding agents (skills + MCP)
    #[command(long_about = "
Set up SwissArmyHammer for all detected AI coding agents.

This command:
1. Registers sah as an MCP server for all detected agents (Claude Code, Cursor, Windsurf, etc.)
2. Creates the .sah/ project directory
3. Installs builtin skills to the central .skills/ store with symlinks to each agent

The command is idempotent - safe to run multiple times.

Targets:
  project   Write to project-level config files (default, shared with team via git)
  local     Write to ~/.claude.json per-project config (personal, not committed)
  user      Write to global config files (all projects)

Examples:
  sah init              # Project-level setup (default)
  sah init user         # Global setup for all projects
  sah init local        # Personal setup, not committed to git
")]
    Init {
        /// Where to install the MCP server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove sah from all detected AI coding agents (skills + MCP)
    #[command(long_about = "
Remove SwissArmyHammer from all detected AI coding agents.

By default, only the MCP server entries are removed from agent config files.
Use --remove-directory to also delete .sah/ and installed skills.

Examples:
  sah deinit                     # Remove from project settings
  sah deinit user                # Remove from user settings
  sah deinit --remove-directory  # Also remove .sah/ and skills
")]
    Deinit {
        /// Where to remove the MCP server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
        /// Also remove .sah/ project directory
        #[arg(long)]
        remove_directory: bool,
    },
    /// Diagnose configuration and setup issues
    #[command(long_about = include_str!("commands/doctor/description.md"))]
    Doctor {},
    /// Generate shell completion scripts
    #[command(long_about = "
Generates shell completion scripts for various shells. Supports:
- bash
- zsh
- fish
- powershell

Examples:
  # Bash (add to ~/.bashrc or ~/.bash_profile)
  sah completion bash > ~/.local/share/bash-completion/completions/sah

  # Zsh (add to ~/.zshrc or a file in fpath)
  sah completion zsh > ~/.zfunc/_sah

  # Fish
  sah completion fish > ~/.config/fish/completions/sah.fish

  # PowerShell
  sah completion powershell >> $PROFILE
")]
    Completion {
        /// Shell to generate completion for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Validate skills and workflows for syntax and best practices
    #[command(long_about = include_str!("commands/validate/description.md"))]
    Validate {
        /// Suppress all output except errors. In quiet mode, warnings are hidden from both output and summary.
        #[arg(short, long)]
        quiet: bool,

        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Validate MCP tool schemas for CLI compatibility
        #[arg(long)]
        validate_tools: bool,
    },

    /// Manage tool enable/disable state
    #[command(long_about = "
Manage which MCP tools are enabled or disabled.

Tools are enabled by default. Disable tools you don't need to reduce
the tool surface visible to AI agents.

Examples:
  sah tools                          # List all tools with status
  sah tools disable                  # Disable all tools
  sah tools enable shell git         # Enable specific tools
  sah tools disable kanban web       # Disable specific tools
  sah tools enable                   # Enable all tools
  sah tools --global disable web     # Disable web globally
")]
    Tools {
        /// Write to global config (~/.sah/tools.yaml) instead of project
        #[arg(long)]
        global: bool,

        #[command(subcommand)]
        subcommand: Option<ToolsSubcommand>,
    },

    /// Render statusline from Claude Code JSON (stdin) or dump config
    #[command(long_about = "
Render a styled statusline for Claude Code integration.

In normal mode, reads JSON from stdin and outputs styled ANSI text.
Use 'sah statusline config' to dump the full annotated builtin config.

The statusline is configured via YAML with 3-layer stacking:
  1. Builtin defaults (embedded in binary)
  2. User config (~/.sah/statusline/config.yaml)
  3. Project config (.sah/statusline/config.yaml)

Examples:
  echo '{\"model\":{\"display_name\":\"Opus\"}}' | sah statusline
  sah statusline config > .sah/statusline/config.yaml
")]
    Statusline {
        #[command(subcommand)]
        subcommand: Option<StatuslineSubcommand>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServeSubcommand {
    /// Start HTTP MCP server
    #[command(long_about = "
Start HTTP MCP server for web clients, debugging, and ACP agent integration.
The server exposes MCP tools through HTTP endpoints and provides:

- RESTful MCP protocol implementation
- Health check endpoint at /health
- Support for random port allocation (use port 0)
- Graceful shutdown with Ctrl+C

Example:
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  swissarmyhammer serve http --port 0  # Random port
")]
    Http {
        /// Port to bind to (use 0 for random port)
        #[arg(long, short = 'p', default_value = "8000", value_parser = clap::value_parser!(u16))]
        port: u16,

        /// Host to bind to
        #[arg(long, short = 'H', default_value = "127.0.0.1")]
        host: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum StatuslineSubcommand {
    /// Dump the full annotated builtin config to stdout
    Config,
}

#[derive(Subcommand, Debug)]
pub enum ToolsSubcommand {
    /// Enable tools (all if no names given)
    Enable {
        /// Tool names to enable (omit for all)
        names: Vec<String>,
    },
    /// Disable tools (all if no names given)
    Disable {
        /// Tool names to disable (omit for all)
        names: Vec<String>,
    },
}

impl Cli {
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

    #[test]
    fn test_cli_help_works() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--help"]);
        assert!(result.is_err()); // Help exits with error code but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_cli_version_works() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--version"]);
        assert!(result.is_err()); // Version exits with error code but that's expected

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_cli_no_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_cli_serve_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "serve"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Serve { subcommand: _ })
        ));
    }

    #[test]
    fn test_cli_init_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_init_user() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init", "user"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::User
            })
        ));
    }

    #[test]
    fn test_cli_init_local() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "init", "local"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Local
            })
        ));
    }

    #[test]
    fn test_cli_deinit_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "deinit"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::Project);
            assert!(!remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_deinit_with_remove_directory() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "deinit", "--remove-directory"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::Project);
            assert!(remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_deinit_user_with_remove_directory() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "deinit", "user", "--remove-directory"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Deinit {
            target,
            remove_directory,
        }) = cli.command
        {
            assert_eq!(target, InstallTarget::User);
            assert!(remove_directory);
        } else {
            unreachable!("Expected Deinit command");
        }
    }

    #[test]
    fn test_cli_doctor_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.command, Some(Commands::Doctor {})));
    }

    #[test]
    fn test_cli_verbose_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_cli_quiet_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--quiet"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);
    }

    #[test]
    fn test_cli_serve_with_verbose() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--verbose", "serve"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(
            cli.command,
            Some(Commands::Serve { subcommand: _ })
        ));
    }

    #[test]
    fn test_cli_invalid_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "invalid"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn test_cli_validate_command() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "validate"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Validate {
            quiet,
            format,
            validate_tools: _,
        }) = cli.command
        {
            assert!(!quiet);
            assert!(matches!(format, OutputFormat::Table));
        } else {
            unreachable!("Expected Validate command");
        }
    }

    #[test]
    fn test_cli_validate_command_with_options() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "validate",
            "--quiet",
            "--format",
            "json",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Validate {
            quiet,
            format,
            validate_tools: _,
        }) = cli.command
        {
            assert!(quiet);
            assert!(matches!(format, OutputFormat::Json));
        } else {
            unreachable!("Expected Validate command");
        }
    }

    #[test]
    fn test_parse_args_panics_on_error() {
        // This test verifies that parse_args would panic on invalid input
        // We can't easily test the panic itself in unit tests, but we can verify
        // that the underlying try_parse_from_args returns an error
        let result = Cli::try_parse_from_args(["swissarmyhammer", "invalid-command"]);
        assert!(result.is_err());
    }

    // `test_source_arg_conversions` moved to `crate::cli_conversions::tests`
    // because it tests conversions to/from `swissarmyhammer_common::file_loader::FileSource`,
    // which lives with the From impls to keep `cli.rs` self-contained.

    #[test]
    fn test_source_arg_equality() {
        assert_eq!(SourceArg::Builtin, SourceArg::Builtin);
        assert_ne!(SourceArg::Builtin, SourceArg::User);
        assert_ne!(SourceArg::User, SourceArg::Local);
        assert_ne!(SourceArg::Local, SourceArg::Dynamic);
    }

    #[test]
    fn test_debug_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--debug"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_combined_flags() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--debug", "--verbose"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.debug);
        assert!(cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_global_format_flag() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "json", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn test_global_format_flag_yaml() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "yaml", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Yaml)));
    }

    #[test]
    fn test_global_format_flag_table() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "table", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.format, Some(OutputFormat::Table)));
    }

    #[test]
    fn test_global_format_flag_default() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "doctor"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        // When global format is not specified, it should be None
        assert_eq!(cli.format, None);
    }

    #[test]
    fn test_global_format_flag_with_verbose() {
        let result = Cli::try_parse_from_args([
            "swissarmyhammer",
            "--verbose",
            "--format",
            "json",
            "doctor",
        ]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn test_global_format_flag_invalid() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "--format", "invalid", "doctor"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tools_no_subcommand() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools { global, subcommand }) = cli.command {
            assert!(!global);
            assert!(subcommand.is_none());
        } else {
            unreachable!("Expected Tools command");
        }
    }

    #[test]
    fn test_tools_enable_multiple_names() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "tools", "enable", "shell", "git"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert_eq!(names, vec!["shell", "git"]);
        } else {
            unreachable!("Expected Tools Enable command with names");
        }
    }

    #[test]
    fn test_tools_disable_single_name() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools", "disable", "kanban"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Disable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert_eq!(names, vec!["kanban"]);
        } else {
            unreachable!("Expected Tools Disable command with name");
        }
    }

    #[test]
    fn test_tools_global_flag_with_enable() {
        let result =
            Cli::try_parse_from_args(["swissarmyhammer", "tools", "--global", "enable", "shell"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(global);
            assert_eq!(names, vec!["shell"]);
        } else {
            unreachable!("Expected Tools Enable command with global flag");
        }
    }

    #[test]
    fn test_tools_enable_no_names() {
        let result = Cli::try_parse_from_args(["swissarmyhammer", "tools", "enable"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        if let Some(Commands::Tools {
            global,
            subcommand: Some(ToolsSubcommand::Enable { names }),
        }) = cli.command
        {
            assert!(!global);
            assert!(names.is_empty());
        } else {
            unreachable!("Expected Tools Enable command with empty names");
        }
    }
}
