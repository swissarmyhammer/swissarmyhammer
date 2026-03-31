//! shelltool CLI — Standalone MCP shell tool for AI coding agents.
//!
//! Commands:
//! - `shelltool serve`: Run MCP server over stdio, exposing the shell tool
//! - `shelltool init [target]`: Install shelltool into Claude Code settings
//! - `shelltool deinit [target]`: Remove shelltool from Claude Code settings
//! - `shelltool doctor`: Diagnose shelltool setup
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error

use clap::Parser;
use std::sync::{Arc, Mutex};
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;
use swissarmyhammer_directory::{DirectoryConfig, ShellConfig};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

mod banner;
mod cli;
mod doctor;
mod registry;
mod serve;

use cli::{Cli, Commands, InstallTarget};

/// Writer that flushes and syncs on every write for reliable log output.
struct FileWriterGuard {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    fn new(file: Arc<Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().expect("log file mutex poisoned");
        let result = file.write(buf)?;
        file.flush()?;
        file.sync_all()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().expect("log file mutex poisoned");
        file.flush()?;
        file.sync_all()
    }
}

#[tokio::main]
async fn main() {
    // Show banner for interactive help invocations
    {
        let args: Vec<String> = std::env::args().collect();
        if banner::should_show_banner(&args) {
            banner::print_banner();
        }
    }

    let cli = Cli::parse();

    // Configure tracing: file-based logging to .shell/mcp.log, matching sah's approach
    let make_filter = || -> EnvFilter {
        if cli.debug {
            EnvFilter::new(
                "shelltool=debug,swissarmyhammer_tools=debug,swissarmyhammer_shell=debug",
            )
        } else {
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rmcp=warn,debug"))
        }
    };

    let log_dir = std::path::PathBuf::from(ShellConfig::DIR_NAME);
    let log_configured = if std::fs::create_dir_all(&log_dir).is_ok() {
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
                .with_filter(make_filter());
            tracing_subscriber::registry().with(file_layer).init();
            true
        } else {
            false
        }
    } else {
        false
    };

    // Fallback to stderr if file logging couldn't be set up
    if !log_configured {
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_ansi(false)
            .with_writer(std::io::stderr)
            .with_filter(make_filter());
        tracing_subscriber::registry().with(stderr_layer).init();
    }

    let exit_code = dispatch_command(cli).await;
    std::process::exit(exit_code);
}

/// Dispatch the parsed CLI command to the appropriate handler.
///
/// Returns an exit code: 0 for success, 1 for error.
async fn dispatch_command(cli: Cli) -> i32 {
    match cli.command {
        Commands::Serve => match serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
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
            if had_error {
                1
            } else {
                0
            }
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
            if had_error {
                1
            } else {
                0
            }
        }
        Commands::Doctor { verbose } => doctor::run_doctor(verbose),
    }
}
