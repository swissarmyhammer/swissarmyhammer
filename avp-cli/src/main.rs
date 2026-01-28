//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Commands:
//! - `avp` (no args): Read JSON from stdin, process hook, write JSON to stdout
//! - `avp install <target>`: Install AVP hooks into Claude Code settings
//! - `avp uninstall <target>`: Remove AVP hooks from Claude Code settings
//! - `avp doctor`: Diagnose AVP configuration and setup
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
    Install {
        /// Where to install the hooks
        #[arg(value_enum)]
        target: InstallTarget,
    },
    /// Remove AVP hooks from Claude Code settings
    Uninstall {
        /// Where to remove the hooks from
        #[arg(value_enum)]
        target: InstallTarget,
    },
    /// Diagnose AVP configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
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
        Some(Commands::Install { target }) => match install::install(target) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },
        Some(Commands::Uninstall { target }) => match install::uninstall(target) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },
        Some(Commands::Doctor { verbose }) => doctor::run_doctor(verbose),
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
        println!("  avp install <project|local|user>   Install hooks to Claude settings");
        println!("  avp uninstall <project|local|user> Remove hooks from Claude settings");
        println!();
        println!("Examples:");
        println!("  avp install project           Install to .claude/settings.json");
        println!("  avp install user              Install to ~/.claude/settings.json");
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
    fn test_cli_parsing_install_project() {
        let cli = Cli::parse_from(["avp", "install", "project"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Install {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_install_local() {
        let cli = Cli::parse_from(["avp", "install", "local"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Install {
                target: InstallTarget::Local
            })
        ));
    }

    #[test]
    fn test_cli_parsing_install_user() {
        let cli = Cli::parse_from(["avp", "install", "user"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Install {
                target: InstallTarget::User
            })
        ));
    }

    #[test]
    fn test_cli_parsing_uninstall_project() {
        let cli = Cli::parse_from(["avp", "uninstall", "project"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Uninstall {
                target: InstallTarget::Project
            })
        ));
    }

    #[test]
    fn test_cli_parsing_debug_with_install() {
        let cli = Cli::parse_from(["avp", "--debug", "install", "user"]);
        assert!(cli.debug);
        assert!(matches!(
            cli.command,
            Some(Commands::Install {
                target: InstallTarget::User
            })
        ));
    }
}
