//! code-context CLI -- Standalone MCP code-context tool for AI coding agents.
//!
//! Commands:
//! - `code-context serve`: Run MCP server over stdio, exposing code-context tools
//! - `code-context init [target]`: Install code-context into Claude Code settings
//! - `code-context deinit [target]`: Remove code-context from Claude Code settings
//! - `code-context doctor`: Diagnose code-context setup
//! - `code-context skill`: Deploy code-context skill to agent .skills/ directories
//! - `code-context get|search|list|grep|query|find|build|clear|lsp|detect ...`: Operations
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error

use clap::Parser;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;

mod banner;
mod cli;
mod commands;
mod logging;

use cli::{Cli, Commands, InstallTarget};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    let cli = Cli::parse();
    logging::init_tracing(cli.debug);

    let exit_code = dispatch_command(cli).await;
    std::process::exit(exit_code);
}

/// Dispatch the parsed CLI command to the appropriate handler.
///
/// Returns an exit code: 0 for success, 1 for error.
async fn dispatch_command(cli: Cli) -> i32 {
    let json_output = cli.json;

    match cli.command {
        Commands::Serve => match commands::serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e:#}");
                1
            }
        },
        Commands::Init { target } => {
            let scope = match target {
                InstallTarget::Project => InitScope::Project,
                InstallTarget::Local => InitScope::Local,
                InstallTarget::User => InitScope::User,
            };
            let mut reg = InitRegistry::new();
            commands::registry::register_all(&mut reg);
            let reporter = CliReporter;
            let results = reg.run_all_init(&scope, &reporter);
            let had_error = results
                .iter()
                .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error);
            i32::from(had_error)
        }
        Commands::Deinit { target } => {
            let scope = match target {
                InstallTarget::Project => InitScope::Project,
                InstallTarget::Local => InitScope::Local,
                InstallTarget::User => InitScope::User,
            };
            let mut reg = InitRegistry::new();
            commands::registry::register_all(&mut reg);
            let reporter = CliReporter;
            let results = reg.run_all_deinit(&scope, &reporter);
            let had_error = results
                .iter()
                .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error);
            i32::from(had_error)
        }
        Commands::Doctor { verbose } => commands::doctor::run_doctor(verbose),
        Commands::Skill => commands::skill::run_skill(),
        // All other commands are tool operations dispatched via commands::ops::run_operation.
        ref command => commands::ops::run_operation(command, json_output).await,
    }
}
