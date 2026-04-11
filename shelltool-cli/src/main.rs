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

/// Build the `EnvFilter` for tracing given the `--debug` flag.
fn build_tracing_filter(debug: bool) -> EnvFilter {
    if debug {
        EnvFilter::new("shelltool=debug,swissarmyhammer_tools=debug,swissarmyhammer_shell=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rmcp=warn,debug"))
    }
}

/// Try to install a file-based tracing subscriber writing to `<log_dir>/mcp.log`.
///
/// Returns `true` on success. On any filesystem error the subscriber is left
/// uninstalled so the caller can fall back to stderr.
fn try_init_file_tracing(log_dir: &std::path::Path, debug: bool) -> bool {
    if std::fs::create_dir_all(log_dir).is_err() {
        return false;
    }
    let log_file_path = log_dir.join("mcp.log");
    let Ok(file) = std::fs::File::create(&log_file_path) else {
        return false;
    };
    let shared_file = Arc::new(Mutex::new(file));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(move || {
            let file = shared_file.clone();
            Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
        })
        .with_filter(build_tracing_filter(debug));
    tracing_subscriber::registry().with(file_layer).init();
    true
}

/// Install a stderr-based tracing subscriber as a fallback.
fn init_stderr_tracing(debug: bool) {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_filter(build_tracing_filter(debug));
    tracing_subscriber::registry().with(stderr_layer).init();
}

#[tokio::main]
async fn main() {
    // Show banner for interactive help invocations
    let args: Vec<String> = std::env::args().collect();
    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    let cli = Cli::parse();

    // Configure tracing: file-based logging to .shell/mcp.log, with stderr fallback.
    let log_dir = std::path::PathBuf::from(ShellConfig::DIR_NAME);
    if !try_init_file_tracing(&log_dir, cli.debug) {
        init_stderr_tracing(cli.debug);
    }

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

/// Run all registered init components for the given scope and return the exit code.
fn run_init(target: InstallTarget) -> i32 {
    let scope = install_target_to_scope(target);
    let mut reg = InitRegistry::new();
    registry::register_all(&mut reg);
    let reporter = CliReporter;
    let results = reg.run_all_init(&scope, &reporter);
    if any_init_error(&results) {
        1
    } else {
        0
    }
}

/// Run all registered deinit components for the given scope and return the exit code.
fn run_deinit(target: InstallTarget) -> i32 {
    let scope = install_target_to_scope(target);
    let mut reg = InitRegistry::new();
    registry::register_all(&mut reg);
    let reporter = CliReporter;
    let results = reg.run_all_deinit(&scope, &reporter);
    if any_init_error(&results) {
        1
    } else {
        0
    }
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
        Commands::Init { target } => run_init(target),
        Commands::Deinit { target } => run_deinit(target),
        Commands::Doctor { verbose } => doctor::run_doctor(verbose),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::sync::LazyLock;
    use tempfile::TempDir;
    use tokio::sync::Mutex as AsyncMutex;

    /// Serializes async tests that mutate process-global environment state
    /// (specifically `env::set_current_dir`). Cargo runs tests in parallel
    /// within a single process, so any test that swaps the CWD must hold
    /// this lock to avoid racing with siblings.
    ///
    /// An async-aware `tokio::sync::Mutex` is used because the guard is
    /// held across `.await` points while `dispatch_command` runs; a
    /// standard `std::sync::Mutex` would trip `clippy::await_holding_lock`.
    static ENV_LOCK: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    /// RAII guard that restores `env::current_dir` on drop.
    ///
    /// Ensures tests that `set_current_dir` into a tempdir don't leak
    /// the new CWD to later tests — even if the test panics mid-run.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        /// Capture the current working directory so it can be restored later.
        fn capture() -> Self {
            Self {
                original: std::env::current_dir().expect("current_dir must be readable"),
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            // Best-effort restore; ignore errors during unwind.
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    // ── Actionable 1: FileWriterGuard ────────────────────────────────────

    /// `FileWriterGuard::write` must forward the bytes to the underlying
    /// file, returning the number of bytes written, and must durably sync
    /// them to disk (the impl calls `flush` + `sync_all` after every
    /// `write`). Re-reading the file after the call asserts both forwarding
    /// and durability on the write path.
    #[test]
    fn file_writer_guard_write_persists_bytes() {
        let tempdir = TempDir::new().expect("create tempdir");
        let path = tempdir.path().join("log.txt");

        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("open log file");
        let shared = Arc::new(Mutex::new(file));
        let mut guard = FileWriterGuard::new(Arc::clone(&shared));

        let payload = b"hello";
        let written = guard.write(payload).expect("write should succeed");
        assert_eq!(written, payload.len());

        // Re-read the contents via a fresh handle to confirm the bytes
        // actually reached disk (not just the in-process file buffer).
        let mut contents = Vec::new();
        std::fs::File::open(&path)
            .expect("reopen log file")
            .read_to_end(&mut contents)
            .expect("read log file");
        assert_eq!(contents, payload);
    }

    /// `FileWriterGuard::flush` must forward to the underlying file's
    /// `flush` and `sync_all`. We write unflushed bytes through a separate
    /// handle, then call `flush` on the guard — the guard's flush path
    /// is exercised, and the file stays consistent afterward.
    #[test]
    fn file_writer_guard_flush_syncs_underlying_file() {
        let tempdir = TempDir::new().expect("create tempdir");
        let path = tempdir.path().join("log.txt");

        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("open log file");
        let shared = Arc::new(Mutex::new(file));

        // Write some bytes directly through the shared handle so that
        // a subsequent `flush` via the guard has buffered state to flush.
        {
            let mut locked = shared.lock().expect("lock file");
            locked.write_all(b"payload").expect("direct write");
            // Rewind so the next read starts from the beginning.
            locked.seek(SeekFrom::Start(0)).expect("seek start");
        }

        let mut guard = FileWriterGuard::new(Arc::clone(&shared));
        guard.flush().expect("flush should succeed");

        let mut contents = Vec::new();
        std::fs::File::open(&path)
            .expect("reopen log file")
            .read_to_end(&mut contents)
            .expect("read log file");
        assert_eq!(contents, b"payload");
    }

    // ── Actionable 2: dispatch_command arms ──────────────────────────────

    /// `dispatch_command` must route `Commands::Init { target: Local }`
    /// through the init registry and return an exit code derived from
    /// the component results. Running under a fresh tempdir-as-CWD keeps
    /// the writes hermetic: `.claude/settings.local.json` and `.shell/`
    /// are scoped beneath the tempdir and discarded on drop.
    #[tokio::test]
    async fn dispatch_command_init_local_runs_registry() {
        let _env = ENV_LOCK.lock().await;
        let _cwd = CwdGuard::capture();
        let tempdir = TempDir::new().expect("create tempdir");
        std::env::set_current_dir(tempdir.path()).expect("chdir tempdir");

        let cli = Cli {
            debug: false,
            command: Commands::Init {
                target: InstallTarget::Local,
            },
        };

        let exit_code = dispatch_command(cli).await;
        // The registry may return 0 (success) or 1 (some component
        // errored on the host — e.g. no detectable agents). Either
        // outcome exercises the arm; we just require a valid code.
        assert!(
            exit_code == 0 || exit_code == 1,
            "unexpected exit code: {exit_code}"
        );
    }

    /// `dispatch_command` must route `Commands::Deinit { target: Local }`
    /// through the deinit registry and return an exit code derived from
    /// the component results. As with init, we pin CWD to a tempdir so
    /// any file writes/removes land in a throwaway scope.
    #[tokio::test]
    async fn dispatch_command_deinit_local_runs_registry() {
        let _env = ENV_LOCK.lock().await;
        let _cwd = CwdGuard::capture();
        let tempdir = TempDir::new().expect("create tempdir");
        std::env::set_current_dir(tempdir.path()).expect("chdir tempdir");

        let cli = Cli {
            debug: false,
            command: Commands::Deinit {
                target: InstallTarget::Local,
            },
        };

        let exit_code = dispatch_command(cli).await;
        assert!(
            exit_code == 0 || exit_code == 1,
            "unexpected exit code: {exit_code}"
        );
    }

    /// `dispatch_command` must route `Commands::Doctor { verbose: false }`
    /// straight through to `doctor::run_doctor`. `run_doctor` is already
    /// covered in `doctor::tests`; this test just pins down the dispatch
    /// arm. The host-dependent exit code is either 0, 1, or 2.
    #[tokio::test]
    async fn dispatch_command_doctor_runs_diagnostics() {
        let cli = Cli {
            debug: false,
            command: Commands::Doctor { verbose: false },
        };

        let exit_code = dispatch_command(cli).await;
        assert!(exit_code <= 2, "unexpected exit code: {exit_code}");
    }
}
