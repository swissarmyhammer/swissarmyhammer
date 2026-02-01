//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Commands:
//! - `avp` (no args): Read JSON from stdin, process hook, write JSON to stdout
//! - `avp init [target]`: Install AVP hooks into Claude Code settings (default: project)
//! - `avp deinit [target]`: Remove AVP hooks and .avp directory (default: project)
//! - `avp doctor`: Diagnose AVP configuration and setup
//! - `avp list`: List all available validators
//!
//! Targets: project, local, user
//!
//! Exit codes:
//! - 0: Success
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use avp::doctor;
use avp::install::{self, InstallTarget};
use avp::list;
use avp_common::context::AvpContext;
use avp_common::strategy::HookDispatcher;
use avp_common::AvpError;

/// AVP - Agent Validator Protocol
///
/// Claude Code hook processor that validates tool calls, file changes, and more.
#[derive(Parser, Debug)]
#[command(name = "avp")]
#[command(version)]
#[command(about = "Agent Validator Protocol - Claude Code hook processor")]
struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
    /// List all available validators
    List {
        /// Show detailed output including descriptions
        #[arg(short, long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing with appropriate level
    let filter = if cli.debug {
        EnvFilter::new("avp=debug,avp_common=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    let exit_code = match cli.command {
        Some(Commands::Init { target }) => match install::install(target) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },
        Some(Commands::Deinit { target }) => match install::uninstall(target) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },
        Some(Commands::Doctor { verbose }) => doctor::run_doctor(verbose),
        Some(Commands::List { verbose }) => list::run_list(verbose, cli.debug),
        None => {
            // Default behavior: process hook from stdin
            match run_hook_processor(&cli).await {
                Ok(code) => code,
                Err(e) => {
                    // Output error as JSON for consistency
                    let error_output = serde_json::json!({
                        "continue": false,
                        "stopReason": e.to_string()
                    });
                    tracing::error!("{}", e);
                    let _ = io::stdout().write_all(error_output.to_string().as_bytes());
                    2 // Blocking error
                }
            }
        }
    };

    std::process::exit(exit_code);
}

async fn run_hook_processor(_cli: &Cli) -> Result<i32, AvpError> {
    // Check if stdin is a terminal (no piped input)
    if io::stdin().is_terminal() {
        // Show help when run interactively without subcommand
        println!("AVP - Agent Validator Protocol");
        println!();
        println!("Usage:");
        println!("  avp                           Process hook from stdin (pipe JSON)");
        println!("  avp init [project|local|user]      Install hooks (default: project)");
        println!("  avp deinit [project|local|user]    Remove hooks and .avp dir (default: project)");
        println!("  avp list [-v]                 List all available validators");
        println!("  avp doctor [-v]               Diagnose AVP setup");
        println!();
        println!("Examples:");
        println!("  avp init                      Install to .claude/settings.json");
        println!("  avp init user                 Install to ~/.claude/settings.json");
        println!("  avp list                      Show all validators");
        println!("  avp list -v                   Show validators with descriptions");
        println!("  echo '{{...}}' | avp           Process a hook event");
        println!();
        println!("Run 'avp --help' for more information.");
        return Ok(0);
    }

    // Read JSON from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    // Handle empty input
    let input_str = input_str.trim();
    if input_str.is_empty() {
        tracing::warn!("no input provided");
        return Ok(0);
    }

    tracing::debug!("Input: {}", input_str);

    // Parse input JSON
    let input_value: serde_json::Value = serde_json::from_str(input_str)?;

    // Extract hook event name for debug logging
    let hook_event_name: String = input_value
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    // Initialize context (required for dispatcher)
    let ctx = AvpContext::init()?;

    // Create dispatcher with default strategies, passing the context
    let dispatcher = HookDispatcher::with_defaults(ctx);

    // Process the hook (async) - logging is handled by ClaudeCodeHookStrategy
    let (output, exit_code) = dispatcher.dispatch(input_value).await?;

    tracing::debug!(
        hook = %hook_event_name,
        exit_code = exit_code,
        "Hook processed"
    );

    // Write JSON to stdout with trailing newline
    let output_json = serde_json::to_string(&output)?;
    io::stdout().write_all(output_json.as_bytes())?;
    io::stdout().write_all(b"\n")?;

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_no_args() {
        let cli = Cli::parse_from(["avp"]);
        assert!(!cli.debug);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_parsing_debug() {
        let cli = Cli::parse_from(["avp", "--debug"]);
        assert!(cli.debug);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_parsing_init_default() {
        let cli = Cli::parse_from(["avp", "init"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_init_project() {
        let cli = Cli::parse_from(["avp", "init", "project"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_init_local() {
        let cli = Cli::parse_from(["avp", "init", "local"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::Local
            })
        ));
    }

    #[test]
    fn test_cli_parsing_init_user() {
        let cli = Cli::parse_from(["avp", "init", "user"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::User
            })
        ));
    }

    #[test]
    fn test_cli_parsing_deinit_default() {
        let cli = Cli::parse_from(["avp", "deinit"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Deinit {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_deinit_project() {
        let cli = Cli::parse_from(["avp", "deinit", "project"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Deinit {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_debug_with_init() {
        let cli = Cli::parse_from(["avp", "--debug", "init", "user"]);
        assert!(cli.debug);
        assert!(matches!(
            cli.command,
            Some(Commands::Init {
                target: InstallTarget::User
            })
        ));
    }

    #[test]
    fn test_cli_parsing_list() {
        let cli = Cli::parse_from(["avp", "list"]);
        assert!(matches!(
            cli.command,
            Some(Commands::List { verbose: false })
        ));
    }

    #[test]
    fn test_cli_parsing_list_verbose() {
        let cli = Cli::parse_from(["avp", "list", "-v"]);
        assert!(matches!(
            cli.command,
            Some(Commands::List { verbose: true })
        ));
    }

    #[test]
    fn test_cli_parsing_list_verbose_long() {
        let cli = Cli::parse_from(["avp", "list", "--verbose"]);
        assert!(matches!(
            cli.command,
            Some(Commands::List { verbose: true })
        ));
    }
}
