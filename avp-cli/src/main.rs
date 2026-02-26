//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Commands:
//! - `avp` (no args): Read JSON from stdin, process hook, write JSON to stdout
//! - `avp init [target]`: Install AVP hooks into Claude Code settings (default: project)
//! - `avp deinit [target]`: Remove AVP hooks and .avp directory (default: project)
//! - `avp doctor`: Diagnose AVP configuration and setup
//! - `avp new <name>`: Create a new RuleSet from template
//! - `avp edit <name>`: Edit an existing RuleSet in $EDITOR
//!
//! Targets: project, local, user
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::{CommandFactory, Parser};
use tracing_subscriber::EnvFilter;

mod banner;

/// Exit code returned when a hook blocks the action.
const BLOCKING_ERROR_EXIT_CODE: i32 = 2;

use avp::{doctor, edit, install, new};
use avp::{Cli, Commands};
use avp_common::context::AvpContext;
use avp_common::strategy::HookDispatcher;
use avp_common::AvpError;

#[tokio::main]
async fn main() {
    // Show branded banner for top-level help (no subcommand or --help/-h).
    {
        let args: Vec<String> = std::env::args().collect();
        let show = match args.len() {
            1 => true,
            2 => args[1] == "--help" || args[1] == "-h",
            _ => false,
        };
        if show {
            banner::print_banner();
        }
    }

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

    let exit_code = dispatch_command(cli).await;
    std::process::exit(exit_code);
}

/// Dispatch a parsed CLI to the appropriate command handler.
async fn dispatch_command(cli: Cli) -> i32 {
    let debug = cli.debug;
    match cli.command {
        Some(cmd) => dispatch_subcommand(cmd, debug).await,
        None => run_hook_or_error(&cli).await,
    }
}

/// Handle an explicit subcommand.
async fn dispatch_subcommand(cmd: Commands, _debug: bool) -> i32 {
    match cmd {
        Commands::Init { target } => result_to_exit(install::install(target)),
        Commands::Deinit { target } => result_to_exit(install::uninstall(target)),
        Commands::Doctor { verbose } => doctor::run_doctor(verbose),
        Commands::Edit { name, global, .. } => result_to_exit(edit::run_edit(&name, global)),
        Commands::New { name, global, .. } => result_to_exit(new::run_new(&name, global)),
    }
}

/// Convert a `Result<(), E: Display>` to an exit code.
fn result_to_exit<E: std::fmt::Display>(result: Result<(), E>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Run the hook processor or return an error code.
async fn run_hook_or_error(cli: &Cli) -> i32 {
    match run_hook_processor(cli).await {
        Ok(code) => code,
        Err(e) => {
            let error_output = serde_json::json!({
                "continue": false,
                "stopReason": e.to_string()
            });
            tracing::error!("{}", e);
            // Best-effort: write error JSON to stdout for the hook caller.
            let _ = io::stdout().write_all(error_output.to_string().as_bytes());
            BLOCKING_ERROR_EXIT_CODE
        }
    }
}

async fn run_hook_processor(_cli: &Cli) -> Result<i32, AvpError> {
    // Check if stdin is a terminal (no piped input)
    if io::stdin().is_terminal() {
        // Show clap-generated help when run interactively without subcommand
        Cli::command().print_help().ok();
        println!();
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
    use avp::install::InstallTarget;

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
    fn test_cli_parsing_edit() {
        let cli = Cli::parse_from(["avp", "edit", "my-ruleset"]);
        match cli.command {
            Some(Commands::Edit {
                name,
                global,
                local,
            }) => {
                assert_eq!(name, "my-ruleset");
                assert!(!global);
                assert!(!local);
            }
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn test_cli_parsing_edit_local() {
        let cli = Cli::parse_from(["avp", "edit", "my-ruleset", "--local"]);
        match cli.command {
            Some(Commands::Edit { local, global, .. }) => {
                assert!(local);
                assert!(!global);
            }
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn test_cli_parsing_edit_project_alias() {
        let cli = Cli::parse_from(["avp", "edit", "my-ruleset", "--project"]);
        match cli.command {
            Some(Commands::Edit { local, .. }) => assert!(local),
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn test_cli_parsing_edit_global() {
        let cli = Cli::parse_from(["avp", "edit", "my-ruleset", "--global"]);
        match cli.command {
            Some(Commands::Edit { name, global, .. }) => {
                assert_eq!(name, "my-ruleset");
                assert!(global);
            }
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn test_cli_parsing_edit_user_alias() {
        let cli = Cli::parse_from(["avp", "edit", "my-ruleset", "--user"]);
        match cli.command {
            Some(Commands::Edit { global, .. }) => assert!(global),
            _ => panic!("Expected Edit command"),
        }
    }

    #[test]
    fn test_cli_parsing_new() {
        let cli = Cli::parse_from(["avp", "new", "my-validator"]);
        match cli.command {
            Some(Commands::New { name, global, .. }) => {
                assert_eq!(name, "my-validator");
                assert!(!global);
            }
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_local() {
        let cli = Cli::parse_from(["avp", "new", "my-validator", "--local"]);
        match cli.command {
            Some(Commands::New { local, global, .. }) => {
                assert!(local);
                assert!(!global);
            }
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_project_alias() {
        let cli = Cli::parse_from(["avp", "new", "my-validator", "--project"]);
        match cli.command {
            Some(Commands::New { local, .. }) => assert!(local),
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_user_alias() {
        let cli = Cli::parse_from(["avp", "new", "my-validator", "--user"]);
        match cli.command {
            Some(Commands::New { global, .. }) => assert!(global),
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_global() {
        let cli = Cli::parse_from(["avp", "new", "my-validator", "--global"]);
        match cli.command {
            Some(Commands::New { name, global, .. }) => {
                assert_eq!(name, "my-validator");
                assert!(global);
            }
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_blocking_error_exit_code_is_two() {
        assert_eq!(BLOCKING_ERROR_EXIT_CODE, 2);
    }

    #[test]
    fn test_result_to_exit_ok() {
        let result: Result<(), String> = Ok(());
        assert_eq!(result_to_exit(result), 0);
    }

    #[test]
    fn test_result_to_exit_err() {
        let result: Result<(), String> = Err("something failed".to_string());
        assert_eq!(result_to_exit(result), 1);
    }
}
