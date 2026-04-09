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
use std::sync::{Arc, Mutex};
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::logging::FileWriterGuard;
use swissarmyhammer_common::reporter::CliReporter;
use swissarmyhammer_directory::{CodeContextConfig, DirectoryConfig};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

mod banner;
mod cli;
mod doctor;
mod ops;
mod registry;
mod serve;
mod skill;

use cli::{Cli, Commands, InstallTarget};

/// Build an `EnvFilter` based on whether debug mode is enabled.
fn make_filter(debug: bool) -> EnvFilter {
    if debug {
        EnvFilter::new(
            "code_context_cli=debug,swissarmyhammer_tools=debug,swissarmyhammer_code_context=debug",
        )
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rmcp=warn,debug"))
    }
}

/// Initialize tracing with file-based logging to `.code-context/mcp.log`.
///
/// Falls back to stderr if the log file cannot be created.
fn init_tracing(debug: bool) {
    let log_dir = std::path::PathBuf::from(CodeContextConfig::DIR_NAME);
    if std::fs::create_dir_all(&log_dir).is_ok() {
        let log_file_path = log_dir.join("mcp.log");
        if let Ok(file) = std::fs::File::create(&log_file_path) {
            let shared_file = Arc::new(Mutex::new(file));
            let file_layer = tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(move || {
                    let file = shared_file.clone();
                    Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
                })
                .with_filter(make_filter(debug));
            tracing_subscriber::registry().with(file_layer).init();
            return;
        }
    }
    // Fallback to stderr if file logging couldn't be set up.
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_filter(make_filter(debug));
    tracing_subscriber::registry().with(stderr_layer).init();
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    let cli = Cli::parse();
    init_tracing(cli.debug);

    let exit_code = dispatch_command(cli).await;
    std::process::exit(exit_code);
}

/// Dispatch the parsed CLI command to the appropriate handler.
///
/// Returns an exit code: 0 for success, 1 for error.
async fn dispatch_command(cli: Cli) -> i32 {
    let json_output = cli.json;

    match cli.command {
        Commands::Serve => match serve::run_serve().await {
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
            registry::register_all(&mut reg);
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
            registry::register_all(&mut reg);
            let reporter = CliReporter;
            let results = reg.run_all_deinit(&scope, &reporter);
            let had_error = results
                .iter()
                .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error);
            i32::from(had_error)
        }
        Commands::Doctor { verbose } => doctor::run_doctor(verbose),
        Commands::Skill => skill::run_skill(),
        // All other commands are tool operations dispatched via ops::run_operation.
        ref command => ops::run_operation(command, json_output).await,
    }
}
