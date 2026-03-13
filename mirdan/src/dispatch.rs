//! Shared command dispatch logic used by both the CLI and Tauri app binaries.

use crate::registry::RegistryError;
use crate::{agents, auth, doctor, info, install, list, new, outdated, publish, search, sync};
use crate::{Cli, Commands, NewKind};

/// Map a registry result to a process exit code (0 = success, 1 = error).
fn handle_registry_result(result: Result<(), RegistryError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Like [`handle_registry_result`] but for commands that return a status message.
fn handle_registry_result_msg(result: Result<String, RegistryError>) -> i32 {
    match result {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Dispatch a parsed CLI command and return an exit code.
///
/// Returns `None` for `Commands::Start` — callers handle that variant
/// themselves (the CLI prints brew instructions, the app enters tray mode).
pub async fn dispatch(cli: &Cli) -> Option<i32> {
    let agent_filter = cli.agent.as_deref();

    let code = match &cli.command {
        Commands::Agents { all, json } => handle_registry_result(agents::run_agents(*all, *json)),

        Commands::New { kind } => match kind {
            NewKind::Skill { name, global } => {
                handle_registry_result(new::run_new_skill(name, *global, agent_filter))
            }
            NewKind::Validator { name, global } => {
                handle_registry_result(new::run_new_validator(name, *global))
            }
            NewKind::Tool { name, global } => {
                handle_registry_result(new::run_new_tool(name, *global))
            }
            NewKind::Plugin { name, global } => {
                handle_registry_result(new::run_new_plugin(name, *global))
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
            if *mcp {
                let Some(cmd) = command else {
                    unreachable!("clap enforces --command with --mcp")
                };
                handle_registry_result(
                    install::run_install_mcp(package, cmd, args.clone(), agent_filter, *global)
                        .await,
                )
            } else {
                handle_registry_result(
                    install::run_install(package, agent_filter, *global, *git, skill.as_deref())
                        .await,
                )
            }
        }

        Commands::Uninstall { name, global } => {
            handle_registry_result(install::run_uninstall(name, agent_filter, *global).await)
        }

        Commands::List {
            skills,
            validators,
            tools,
            plugins,
            json,
        } => handle_registry_result(list::run_list(
            *skills,
            *validators,
            *tools,
            *plugins,
            agent_filter,
            *json,
        )),

        Commands::Search { query, json } => match query {
            Some(q) => handle_registry_result(search::run_search(q, *json).await),
            None => handle_registry_result(search::run_interactive_search().await),
        },

        Commands::Info { name } => handle_registry_result(info::run_info(name, agent_filter).await),

        Commands::Login => handle_registry_result(auth::login().await),
        Commands::Logout => handle_registry_result(auth::logout().await),
        Commands::Whoami => handle_registry_result(auth::whoami().await),

        Commands::Publish { source, dry_run } => {
            handle_registry_result(publish::run_publish(source, *dry_run).await)
        }

        Commands::Unpublish { name_version } => {
            handle_registry_result(publish::run_unpublish(name_version, cli.yes).await)
        }

        Commands::Outdated => handle_registry_result(outdated::run_outdated().await),

        Commands::Update { name, global } => handle_registry_result_msg(
            outdated::run_update(name.as_deref(), agent_filter, *global).await,
        ),

        Commands::Sync { global } => handle_registry_result(sync::run_sync(agent_filter, *global)),

        Commands::Doctor { verbose } => doctor::run_doctor(*verbose).await,

        Commands::Start => return None,
    };

    Some(code)
}
