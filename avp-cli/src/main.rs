//! AVP CLI - Agent Validator Protocol command-line interface.
//!
//! Commands:
//! - `avp` (no args): Read JSON from stdin, process hook, write JSON to stdout
//! - `avp init [target]`: Install AVP hooks into Claude Code settings (default: project)
//! - `avp deinit [target]`: Remove AVP hooks and .avp directory (default: project)
//! - `avp doctor`: Diagnose AVP configuration and setup
//! - `avp list`: List all available validators
//! - `avp login`: Authenticate with the AVP registry
//! - `avp logout`: Log out from the AVP registry
//! - `avp whoami`: Show current authenticated user
//! - `avp search <query>`: Search the registry for packages
//! - `avp info <name>`: Show detailed package information
//! - `avp install <package>`: Install a package from the registry
//! - `avp uninstall <name>`: Remove an installed package
//! - `avp new <name>`: Create a new RuleSet from template
//! - `avp publish`: Publish current directory as a package
//! - `avp unpublish <name@version>`: Remove a published version
//! - `avp outdated`: Check for available updates
//! - `avp update [name]`: Update installed packages
//!
//! Targets: project, local, user
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error
//! - 2: Blocking error (hook rejected the action)

use std::io::{self, IsTerminal, Read, Write};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use avp::install;
use avp::registry::RegistryError;
use avp::{auth, doctor, edit, info, list, new, outdated, package, publish, search};
use avp::{Cli, Commands};
use avp_common::context::AvpContext;
use avp_common::strategy::HookDispatcher;
use avp_common::AvpError;

/// Helper to run an async registry command and map errors to exit codes.
fn handle_registry_result(result: Result<(), RegistryError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
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
        Some(Commands::List {
            verbose,
            global,
            local,
            json,
        }) => list::run_list(verbose, cli.debug, global, local, json),
        Some(Commands::Login) => handle_registry_result(auth::login().await),
        Some(Commands::Logout) => handle_registry_result(auth::logout().await),
        Some(Commands::Whoami) => handle_registry_result(auth::whoami().await),
        Some(Commands::Search { query, tag, json }) => {
            handle_registry_result(search::run_search(&query, tag.as_deref(), json).await)
        }
        Some(Commands::Info { name }) => handle_registry_result(info::run_info(&name).await),
        Some(Commands::Install {
            package, global, ..
        }) => handle_registry_result(package::run_install(&package, global).await),
        Some(Commands::Uninstall { name, global, .. }) => {
            handle_registry_result(package::run_uninstall(&name, global).await)
        }
        Some(Commands::Edit { name, global, .. }) => {
            handle_registry_result(edit::run_edit(&name, global))
        }
        Some(Commands::New { name, global, .. }) => {
            handle_registry_result(new::run_new(&name, global))
        }
        Some(Commands::Publish { path, dry_run }) => {
            handle_registry_result(publish::run_publish(&path, dry_run).await)
        }
        Some(Commands::Unpublish { name_version }) => {
            handle_registry_result(publish::run_unpublish(&name_version).await)
        }
        Some(Commands::Outdated) => handle_registry_result(outdated::run_outdated().await),
        Some(Commands::Update { name, global, .. }) => {
            handle_registry_result(outdated::run_update(name.as_deref(), global).await)
        }
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
        println!("  avp init [project|local|user] Install hooks (default: project)");
        println!("  avp deinit [project|local|user] Remove hooks and .avp dir (default: project)");
        println!("  avp list [-v]                 List all available validators");
        println!("  avp doctor [-v]               Diagnose AVP setup");
        println!();
        println!("Authentication:");
        println!("  avp login                     Authenticate with AVP registry");
        println!("  avp logout                    Log out from AVP registry");
        println!("  avp whoami                    Show current authenticated user");
        println!();
        println!("Package Management:");
        println!("  avp search <query>            Search the registry for packages");
        println!("  avp info <name>               Show detailed package information");
        println!("  avp install <name>[@version]  Install a package from the registry");
        println!("  avp uninstall <name>          Remove an installed package");
        println!("  avp new <name> [--global]     Create a new RuleSet from template");
        println!("  avp publish [path] [--dry-run] Publish a package (default: current dir)");
        println!("  avp unpublish <name>@<ver>    Remove a published version");
        println!("  avp outdated                  Check for available updates");
        println!("  avp update [name]             Update installed packages");
        println!();
        println!("Examples:");
        println!("  avp init                      Install to .claude/settings.json");
        println!("  avp init user                 Install to ~/.claude/settings.json");
        println!("  avp list                      Show all validators");
        println!("  avp list -v                   Show validators with descriptions");
        println!("  avp list --json               Output validators as JSON");
        println!("  avp login                     Log in to registry");
        println!("  avp search security           Search for security validators");
        println!("  avp install no-secrets        Install a package");
        println!("  avp new my-validator          Create a new RuleSet");
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
    fn test_cli_parsing_list() {
        let cli = Cli::parse_from(["avp", "list"]);
        assert!(matches!(
            cli.command,
            Some(Commands::List {
                verbose: false,
                global: false,
                local: false,
                json: false
            })
        ));
    }

    #[test]
    fn test_cli_parsing_list_verbose() {
        let cli = Cli::parse_from(["avp", "list", "-v"]);
        match cli.command {
            Some(Commands::List { verbose, .. }) => assert!(verbose),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_verbose_long() {
        let cli = Cli::parse_from(["avp", "list", "--verbose"]);
        match cli.command {
            Some(Commands::List { verbose, .. }) => assert!(verbose),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_global() {
        let cli = Cli::parse_from(["avp", "list", "--global"]);
        match cli.command {
            Some(Commands::List { global, .. }) => assert!(global),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_json() {
        let cli = Cli::parse_from(["avp", "list", "--json"]);
        match cli.command {
            Some(Commands::List { json, .. }) => assert!(json),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_login() {
        let cli = Cli::parse_from(["avp", "login"]);
        assert!(matches!(cli.command, Some(Commands::Login)));
    }

    #[test]
    fn test_cli_parsing_logout() {
        let cli = Cli::parse_from(["avp", "logout"]);
        assert!(matches!(cli.command, Some(Commands::Logout)));
    }

    #[test]
    fn test_cli_parsing_whoami() {
        let cli = Cli::parse_from(["avp", "whoami"]);
        assert!(matches!(cli.command, Some(Commands::Whoami)));
    }

    #[test]
    fn test_cli_parsing_debug_with_login() {
        let cli = Cli::parse_from(["avp", "--debug", "login"]);
        assert!(cli.debug);
        assert!(matches!(cli.command, Some(Commands::Login)));
    }

    #[test]
    fn test_cli_parsing_search() {
        let cli = Cli::parse_from(["avp", "search", "security"]);
        match cli.command {
            Some(Commands::Search { query, tag, json }) => {
                assert_eq!(query, "security");
                assert_eq!(tag, None);
                assert!(!json);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_search_with_tag() {
        let cli = Cli::parse_from(["avp", "search", "test", "--tag", "security"]);
        match cli.command {
            Some(Commands::Search { query, tag, .. }) => {
                assert_eq!(query, "test");
                assert_eq!(tag, Some("security".to_string()));
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_search_json() {
        let cli = Cli::parse_from(["avp", "search", "test", "--json"]);
        match cli.command {
            Some(Commands::Search { json, .. }) => assert!(json),
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_info() {
        let cli = Cli::parse_from(["avp", "info", "no-secrets"]);
        match cli.command {
            Some(Commands::Info { name }) => assert_eq!(name, "no-secrets"),
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_cli_parsing_install() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets"]);
        match cli.command {
            Some(Commands::Install {
                package,
                global,
                local,
            }) => {
                assert_eq!(package, "no-secrets");
                assert!(!global);
                assert!(!local);
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_local() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets", "--local"]);
        match cli.command {
            Some(Commands::Install { local, global, .. }) => {
                assert!(local);
                assert!(!global);
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_project_alias() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets", "--project"]);
        match cli.command {
            Some(Commands::Install { local, .. }) => assert!(local),
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_user_alias() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets", "--user"]);
        match cli.command {
            Some(Commands::Install { global, .. }) => assert!(global),
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_with_version() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets@1.2.3"]);
        match cli.command {
            Some(Commands::Install { package, .. }) => {
                assert_eq!(package, "no-secrets@1.2.3");
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_global() {
        let cli = Cli::parse_from(["avp", "install", "no-secrets", "--global"]);
        match cli.command {
            Some(Commands::Install { global, .. }) => assert!(global),
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_uninstall() {
        let cli = Cli::parse_from(["avp", "uninstall", "no-secrets"]);
        match cli.command {
            Some(Commands::Uninstall { name, global, .. }) => {
                assert_eq!(name, "no-secrets");
                assert!(!global);
            }
            _ => panic!("Expected Uninstall command"),
        }
    }

    #[test]
    fn test_cli_parsing_uninstall_local() {
        let cli = Cli::parse_from(["avp", "uninstall", "no-secrets", "--local"]);
        match cli.command {
            Some(Commands::Uninstall { local, global, .. }) => {
                assert!(local);
                assert!(!global);
            }
            _ => panic!("Expected Uninstall command"),
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
    fn test_cli_parsing_publish() {
        let cli = Cli::parse_from(["avp", "publish"]);
        match cli.command {
            Some(Commands::Publish { path, dry_run }) => {
                assert_eq!(path, std::path::PathBuf::from("."));
                assert!(!dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_publish_with_path() {
        let cli = Cli::parse_from(["avp", "publish", "./my-package"]);
        match cli.command {
            Some(Commands::Publish { path, dry_run }) => {
                assert_eq!(path, std::path::PathBuf::from("./my-package"));
                assert!(!dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_publish_dry_run() {
        let cli = Cli::parse_from(["avp", "publish", "--dry-run"]);
        match cli.command {
            Some(Commands::Publish { path, dry_run }) => {
                assert_eq!(path, std::path::PathBuf::from("."));
                assert!(dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_publish_path_and_dry_run() {
        let cli = Cli::parse_from(["avp", "publish", "../other", "--dry-run"]);
        match cli.command {
            Some(Commands::Publish { path, dry_run }) => {
                assert_eq!(path, std::path::PathBuf::from("../other"));
                assert!(dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_unpublish() {
        let cli = Cli::parse_from(["avp", "unpublish", "my-pkg@1.0.0"]);
        match cli.command {
            Some(Commands::Unpublish { name_version }) => {
                assert_eq!(name_version, "my-pkg@1.0.0");
            }
            _ => panic!("Expected Unpublish command"),
        }
    }

    #[test]
    fn test_cli_parsing_outdated() {
        let cli = Cli::parse_from(["avp", "outdated"]);
        assert!(matches!(cli.command, Some(Commands::Outdated)));
    }

    #[test]
    fn test_cli_parsing_update() {
        let cli = Cli::parse_from(["avp", "update"]);
        match cli.command {
            Some(Commands::Update { name, global, .. }) => {
                assert_eq!(name, None);
                assert!(!global);
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_specific() {
        let cli = Cli::parse_from(["avp", "update", "no-secrets"]);
        match cli.command {
            Some(Commands::Update { name, .. }) => {
                assert_eq!(name, Some("no-secrets".to_string()));
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_global() {
        let cli = Cli::parse_from(["avp", "update", "--global"]);
        match cli.command {
            Some(Commands::Update { global, .. }) => assert!(global),
            _ => panic!("Expected Update command"),
        }
    }
}
