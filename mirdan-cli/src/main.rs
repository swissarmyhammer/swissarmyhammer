//! Mirdan CLI - Universal skill and validator package manager for AI coding agents.
//!
//! Commands:
//! - `mirdan agents`: Detect and list installed AI coding agents
//! - `mirdan new skill <name>`: Scaffold a new skill (agentskills.io spec)
//! - `mirdan new validator <name>`: Scaffold a new validator (AVP spec)
//! - `mirdan install <package>`: Install a skill or validator (type auto-detected)
//! - `mirdan uninstall <name>`: Remove an installed package
//! - `mirdan list`: List installed skills and validators
//! - `mirdan search <query>`: Search the registry
//! - `mirdan info <name>`: Show package details
//! - `mirdan login`: Authenticate with registry
//! - `mirdan logout`: Revoke token and delete credentials
//! - `mirdan whoami`: Show current authenticated user
//! - `mirdan publish [path]`: Publish skill or validator to registry
//! - `mirdan unpublish <name@ver>`: Remove a published version
//! - `mirdan outdated`: Check for newer versions
//! - `mirdan update [name]`: Update installed packages
//! - `mirdan doctor`: Diagnose setup and configuration
//!
//! Environment variables:
//! - MIRDAN_REGISTRY_URL: Override the registry URL (for local testing)
//! - MIRDAN_TOKEN: Provide an auth token
//! - MIRDAN_CREDENTIALS_PATH: Override credentials file location
//! - MIRDAN_AGENTS_CONFIG: Override agents configuration file
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error

use clap::Parser;
use tracing_subscriber::EnvFilter;

use mirdan::registry::RegistryError;
use mirdan::{
    agents, auth, banner, doctor, info, install, list, new, outdated, publish, search, sync,
};
use mirdan::{Cli, Commands, NewKind};

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
    // Show branded banner for interactive help (not when piped).
    {
        use std::io::IsTerminal;
        let args: Vec<String> = std::env::args().collect();
        let show = match args.len() {
            1 => std::io::stdin().is_terminal(),
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
        EnvFilter::new("mirdan=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    // Validate --agent early if provided
    if let Some(ref agent_id) = cli.agent {
        if let Ok(config) = agents::load_agents_config() {
            if let Err(e) = agents::validate_agent_id(&config, agent_id) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    let agent_filter = cli.agent.as_deref();

    let exit_code = match cli.command {
        Commands::Agents { all, json } => handle_registry_result(agents::run_agents(all, json)),

        Commands::New { kind } => match kind {
            NewKind::Skill { name, global } => {
                handle_registry_result(new::run_new_skill(&name, global, agent_filter))
            }
            NewKind::Validator { name, global } => {
                handle_registry_result(new::run_new_validator(&name, global))
            }
            NewKind::Tool { name, global } => {
                handle_registry_result(new::run_new_tool(&name, global))
            }
            NewKind::Plugin { name, global } => {
                handle_registry_result(new::run_new_plugin(&name, global))
            }
        },

        Commands::Install {
            package,
            global,
            git,
            skill,
            mcp,
            command,
            args,
        } => {
            if mcp {
                let cmd = command.expect("--command is required when --mcp is set");
                handle_registry_result(
                    install::run_install_mcp(&package, &cmd, args, agent_filter, global).await,
                )
            } else {
                handle_registry_result(
                    install::run_install(&package, agent_filter, global, git, skill.as_deref())
                        .await,
                )
            }
        }

        Commands::Uninstall { name, global } => {
            handle_registry_result(install::run_uninstall(&name, agent_filter, global).await)
        }

        Commands::List {
            skills,
            validators,
            tools,
            plugins,
            json,
        } => handle_registry_result(list::run_list(
            skills,
            validators,
            tools,
            plugins,
            agent_filter,
            json,
        )),

        Commands::Search { query, json } => match query {
            Some(q) => handle_registry_result(search::run_search(&q, json).await),
            None => handle_registry_result(search::run_interactive_search().await),
        },

        Commands::Info { name } => {
            handle_registry_result(info::run_info(&name, agent_filter).await)
        }

        Commands::Login => handle_registry_result(auth::login().await),
        Commands::Logout => handle_registry_result(auth::logout().await),
        Commands::Whoami => handle_registry_result(auth::whoami().await),

        Commands::Publish { source, dry_run } => {
            handle_registry_result(publish::run_publish(&source, dry_run).await)
        }

        Commands::Unpublish { name_version } => {
            handle_registry_result(publish::run_unpublish(&name_version, cli.yes).await)
        }

        Commands::Outdated => handle_registry_result(outdated::run_outdated().await),

        Commands::Update { name, global } => handle_registry_result(
            outdated::run_update(name.as_deref(), agent_filter, global).await,
        ),

        Commands::Sync { global } => handle_registry_result(sync::run_sync(agent_filter, global)),

        Commands::Doctor { verbose } => doctor::run_doctor(verbose).await,
    };

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_agents() {
        let cli = Cli::parse_from(["mirdan", "agents"]);
        assert!(matches!(
            cli.command,
            Commands::Agents {
                all: false,
                json: false
            }
        ));
    }

    #[test]
    fn test_cli_parsing_agents_all() {
        let cli = Cli::parse_from(["mirdan", "agents", "--all"]);
        assert!(matches!(cli.command, Commands::Agents { all: true, .. }));
    }

    #[test]
    fn test_cli_parsing_agents_json() {
        let cli = Cli::parse_from(["mirdan", "agents", "--json"]);
        assert!(matches!(cli.command, Commands::Agents { json: true, .. }));
    }

    #[test]
    fn test_cli_parsing_new_skill() {
        let cli = Cli::parse_from(["mirdan", "new", "skill", "my-skill"]);
        match cli.command {
            Commands::New {
                kind: NewKind::Skill { name, global },
            } => {
                assert_eq!(name, "my-skill");
                assert!(!global);
            }
            _ => panic!("Expected New Skill command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_skill_global() {
        let cli = Cli::parse_from(["mirdan", "new", "skill", "my-skill", "--global"]);
        match cli.command {
            Commands::New {
                kind: NewKind::Skill { global, .. },
            } => {
                assert!(global);
            }
            _ => panic!("Expected New Skill command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_validator() {
        let cli = Cli::parse_from(["mirdan", "new", "validator", "my-validator"]);
        match cli.command {
            Commands::New {
                kind: NewKind::Validator { name, global },
            } => {
                assert_eq!(name, "my-validator");
                assert!(!global);
            }
            _ => panic!("Expected New Validator command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_tool() {
        let cli = Cli::parse_from(["mirdan", "new", "tool", "my-tool"]);
        match cli.command {
            Commands::New {
                kind: NewKind::Tool { name, global },
            } => {
                assert_eq!(name, "my-tool");
                assert!(!global);
            }
            _ => panic!("Expected New Tool command"),
        }
    }

    #[test]
    fn test_cli_parsing_new_plugin() {
        let cli = Cli::parse_from(["mirdan", "new", "plugin", "my-plugin"]);
        match cli.command {
            Commands::New {
                kind: NewKind::Plugin { name, global },
            } => {
                assert_eq!(name, "my-plugin");
                assert!(!global);
            }
            _ => panic!("Expected New Plugin command"),
        }
    }

    #[test]
    fn test_cli_parsing_install() {
        let cli = Cli::parse_from(["mirdan", "install", "no-secrets"]);
        match cli.command {
            Commands::Install {
                package,
                global,
                git,
                skill,
                mcp,
                ..
            } => {
                assert_eq!(package, "no-secrets");
                assert!(!global);
                assert!(!git);
                assert_eq!(skill, None);
                assert!(!mcp);
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_with_version() {
        let cli = Cli::parse_from(["mirdan", "install", "no-secrets@1.2.3"]);
        match cli.command {
            Commands::Install { package, .. } => {
                assert_eq!(package, "no-secrets@1.2.3");
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_global() {
        let cli = Cli::parse_from(["mirdan", "install", "pkg", "--global"]);
        match cli.command {
            Commands::Install { global, .. } => assert!(global),
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_git_flag() {
        let cli = Cli::parse_from([
            "mirdan",
            "install",
            "--git",
            "https://github.com/owner/repo",
        ]);
        match cli.command {
            Commands::Install { package, git, .. } => {
                assert_eq!(package, "https://github.com/owner/repo");
                assert!(git);
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_skill_flag() {
        let cli = Cli::parse_from(["mirdan", "install", "owner/repo", "--skill", "my-skill"]);
        match cli.command {
            Commands::Install { package, skill, .. } => {
                assert_eq!(package, "owner/repo");
                assert_eq!(skill, Some("my-skill".to_string()));
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_git_and_skill() {
        let cli = Cli::parse_from([
            "mirdan",
            "install",
            "--git",
            "https://github.com/owner/repo",
            "--skill",
            "art",
        ]);
        match cli.command {
            Commands::Install {
                package,
                git,
                skill,
                ..
            } => {
                assert_eq!(package, "https://github.com/owner/repo");
                assert!(git);
                assert_eq!(skill, Some("art".to_string()));
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_uninstall() {
        let cli = Cli::parse_from(["mirdan", "uninstall", "no-secrets"]);
        match cli.command {
            Commands::Uninstall { name, global } => {
                assert_eq!(name, "no-secrets");
                assert!(!global);
            }
            _ => panic!("Expected Uninstall command"),
        }
    }

    #[test]
    fn test_cli_parsing_list() {
        let cli = Cli::parse_from(["mirdan", "list"]);
        match cli.command {
            Commands::List {
                skills,
                validators,
                tools,
                plugins,
                json,
            } => {
                assert!(!skills);
                assert!(!validators);
                assert!(!tools);
                assert!(!plugins);
                assert!(!json);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_skills() {
        let cli = Cli::parse_from(["mirdan", "list", "--skills"]);
        match cli.command {
            Commands::List { skills, .. } => assert!(skills),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_validators() {
        let cli = Cli::parse_from(["mirdan", "list", "--validators"]);
        match cli.command {
            Commands::List { validators, .. } => assert!(validators),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_tools() {
        let cli = Cli::parse_from(["mirdan", "list", "--tools"]);
        match cli.command {
            Commands::List { tools, .. } => assert!(tools),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_plugins() {
        let cli = Cli::parse_from(["mirdan", "list", "--plugins"]);
        match cli.command {
            Commands::List { plugins, .. } => assert!(plugins),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_list_json() {
        let cli = Cli::parse_from(["mirdan", "list", "--json"]);
        match cli.command {
            Commands::List { json, .. } => assert!(json),
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_cli_parsing_search() {
        let cli = Cli::parse_from(["mirdan", "search", "security"]);
        match cli.command {
            Commands::Search { query, json } => {
                assert_eq!(query, Some("security".to_string()));
                assert!(!json);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_search_interactive() {
        let cli = Cli::parse_from(["mirdan", "search"]);
        match cli.command {
            Commands::Search { query, json } => {
                assert_eq!(query, None);
                assert!(!json);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_search_json() {
        let cli = Cli::parse_from(["mirdan", "search", "test", "--json"]);
        match cli.command {
            Commands::Search { json, .. } => assert!(json),
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_cli_parsing_info() {
        let cli = Cli::parse_from(["mirdan", "info", "no-secrets"]);
        match cli.command {
            Commands::Info { name } => assert_eq!(name, "no-secrets"),
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_cli_parsing_login() {
        let cli = Cli::parse_from(["mirdan", "login"]);
        assert!(matches!(cli.command, Commands::Login));
    }

    #[test]
    fn test_cli_parsing_logout() {
        let cli = Cli::parse_from(["mirdan", "logout"]);
        assert!(matches!(cli.command, Commands::Logout));
    }

    #[test]
    fn test_cli_parsing_whoami() {
        let cli = Cli::parse_from(["mirdan", "whoami"]);
        assert!(matches!(cli.command, Commands::Whoami));
    }

    #[test]
    fn test_cli_parsing_debug_global() {
        let cli = Cli::parse_from(["mirdan", "--debug", "agents"]);
        assert!(cli.debug);
    }

    #[test]
    fn test_cli_parsing_publish() {
        let cli = Cli::parse_from(["mirdan", "publish"]);
        match cli.command {
            Commands::Publish { source, dry_run } => {
                assert_eq!(source, ".");
                assert!(!dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_publish_dry_run() {
        let cli = Cli::parse_from(["mirdan", "publish", "--dry-run"]);
        match cli.command {
            Commands::Publish { dry_run, .. } => assert!(dry_run),
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_publish_url() {
        let cli = Cli::parse_from(["mirdan", "publish", "https://github.com/obra/superpowers"]);
        match cli.command {
            Commands::Publish { source, dry_run } => {
                assert_eq!(source, "https://github.com/obra/superpowers");
                assert!(!dry_run);
            }
            _ => panic!("Expected Publish command"),
        }
    }

    #[test]
    fn test_cli_parsing_unpublish() {
        let cli = Cli::parse_from(["mirdan", "unpublish", "my-pkg@1.0.0"]);
        match cli.command {
            Commands::Unpublish { name_version } => assert_eq!(name_version, "my-pkg@1.0.0"),
            _ => panic!("Expected Unpublish command"),
        }
    }

    #[test]
    fn test_cli_parsing_outdated() {
        let cli = Cli::parse_from(["mirdan", "outdated"]);
        assert!(matches!(cli.command, Commands::Outdated));
    }

    #[test]
    fn test_cli_parsing_update() {
        let cli = Cli::parse_from(["mirdan", "update"]);
        match cli.command {
            Commands::Update { name, global, .. } => {
                assert_eq!(name, None);
                assert!(!global);
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_specific() {
        let cli = Cli::parse_from(["mirdan", "update", "no-secrets"]);
        match cli.command {
            Commands::Update { name, .. } => assert_eq!(name, Some("no-secrets".to_string())),
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_global() {
        let cli = Cli::parse_from(["mirdan", "update", "--global"]);
        match cli.command {
            Commands::Update { global, .. } => assert!(global),
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_doctor() {
        let cli = Cli::parse_from(["mirdan", "doctor"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: false }));
    }

    #[test]
    fn test_cli_parsing_doctor_verbose() {
        let cli = Cli::parse_from(["mirdan", "doctor", "--verbose"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: true }));
    }

    #[test]
    fn test_cli_parsing_agent_before_subcommand() {
        let cli = Cli::parse_from(["mirdan", "--agent", "claude-code", "list"]);
        assert_eq!(cli.agent, Some("claude-code".to_string()));
        assert!(matches!(cli.command, Commands::List { .. }));
    }

    #[test]
    fn test_cli_parsing_agent_after_subcommand() {
        let cli = Cli::parse_from(["mirdan", "install", "pkg", "--agent", "cursor"]);
        assert_eq!(cli.agent, Some("cursor".to_string()));
        assert!(matches!(cli.command, Commands::Install { .. }));
    }

    #[test]
    fn test_cli_parsing_no_agent() {
        let cli = Cli::parse_from(["mirdan", "list"]);
        assert_eq!(cli.agent, None);
    }

    #[test]
    fn test_cli_parsing_yes_global() {
        let cli = Cli::parse_from(["mirdan", "--yes", "unpublish", "pkg@1.0.0"]);
        assert!(cli.yes);
    }

    #[test]
    fn test_cli_parsing_yes_short() {
        let cli = Cli::parse_from(["mirdan", "-y", "unpublish", "pkg@1.0.0"]);
        assert!(cli.yes);
    }

    #[test]
    fn test_cli_parsing_yes_after_subcommand() {
        let cli = Cli::parse_from(["mirdan", "unpublish", "pkg@1.0.0", "--yes"]);
        assert!(cli.yes);
    }

    #[test]
    fn test_cli_parsing_no_yes_default() {
        let cli = Cli::parse_from(["mirdan", "list"]);
        assert!(!cli.yes);
    }

    #[test]
    fn test_cli_parsing_install_mcp() {
        let cli = Cli::parse_from([
            "mirdan",
            "install",
            "sah",
            "--mcp",
            "--command",
            "sah",
            "--args",
            "serve",
        ]);
        match cli.command {
            Commands::Install {
                package,
                mcp,
                command,
                args,
                ..
            } => {
                assert_eq!(package, "sah");
                assert!(mcp);
                assert_eq!(command, Some("sah".to_string()));
                assert_eq!(args, vec!["serve"]);
            }
            _ => panic!("Expected Install command"),
        }
    }

    #[test]
    fn test_cli_parsing_install_mcp_requires_command() {
        // --mcp without --command should fail parsing
        let result = Cli::try_parse_from(["mirdan", "install", "sah", "--mcp"]);
        assert!(result.is_err(), "--mcp without --command should fail");
    }

    #[test]
    fn test_cli_parsing_sync() {
        let cli = Cli::parse_from(["mirdan", "sync"]);
        assert!(matches!(cli.command, Commands::Sync { global: false }));
    }

    #[test]
    fn test_cli_parsing_sync_global() {
        let cli = Cli::parse_from(["mirdan", "sync", "--global"]);
        assert!(matches!(cli.command, Commands::Sync { global: true }));
    }
}
