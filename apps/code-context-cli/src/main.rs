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
mod completions;
mod logging;
mod progress;

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

/// Map an `InstallTarget` from the CLI to the corresponding lifecycle `InitScope`.
fn install_target_to_scope(target: InstallTarget) -> InitScope {
    match target {
        InstallTarget::Project => InitScope::Project,
        InstallTarget::Local => InitScope::Local,
        InstallTarget::User => InitScope::User,
    }
}

/// Return `true` if any `InitResult` has `Error` status.
fn any_init_error(results: &[swissarmyhammer_common::lifecycle::InitResult]) -> bool {
    results
        .iter()
        .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error)
}

/// Install code-context for the given scope and return the exit code.
///
/// Runs the mirdan profile installer (registers the `code-context` MCP server —
/// strategy-aware, so it handles Claude local scope the old hand-rolled loop
/// dropped — and deploys the `code-context` + `explore` + `lsp` +
/// `detected-projects` skills) followed by the
/// genuine tool-lifecycle components (the `.code-context/` directory +
/// `.gitignore`). A single errored result from either phase demotes the run to
/// exit code 1.
fn run_init(target: InstallTarget) -> i32 {
    let scope = install_target_to_scope(target);
    let reporter = CliReporter;

    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let results = mirdan::install::init_profile_with_registry(
        &commands::registry::profile(scope),
        &reg,
        scope,
        None,
        &reporter,
    );

    i32::from(any_init_error(&results))
}

/// Remove code-context for the given scope and return the exit code.
///
/// Mirrors [`run_init`]: deinits the genuine tool-lifecycle components, then runs
/// the mirdan profile deinstaller (unregisters the MCP server and removes the
/// `code-context` + `explore` + `lsp` + `detected-projects` skills).
fn run_deinit(target: InstallTarget) -> i32 {
    let scope = install_target_to_scope(target);
    let reporter = CliReporter;

    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
    let results = mirdan::install::deinit_profile_with_registry(
        &commands::registry::profile(scope),
        &reg,
        scope,
        None,
        &reporter,
    );

    i32::from(any_init_error(&results))
}

/// Dispatch the parsed CLI command to the appropriate handler.
///
/// Returns an exit code: 0 for success, 1 for error.
async fn dispatch_command(cli: Cli) -> i32 {
    let json_output = cli.json;
    let no_progress = cli.no_progress;

    match cli.command {
        Commands::Serve => match commands::serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e:#}");
                1
            }
        },
        Commands::Init { target } => run_init(target),
        Commands::Deinit { target } => run_deinit(target),
        Commands::Doctor { verbose } => commands::doctor::run_doctor(verbose).await,
        Commands::Skill => commands::skill::run_skill(),
        Commands::Completion { shell } => match completions::print_completion(shell) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e}");
                1
            }
        },
        // All other commands are tool operations dispatched via commands::ops::run_operation.
        ref command => commands::ops::run_operation(command, json_output, no_progress).await,
    }
}
