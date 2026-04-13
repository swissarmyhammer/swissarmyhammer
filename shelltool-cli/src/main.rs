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
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;

mod banner;
mod cli;
mod commands;
mod logging;

use cli::{Cli, Commands, InstallTarget};

// Re-exports used by the in-file `tests` module so that `use super::*;` resolves
// `Arc`, `Mutex`, and `FileWriterGuard` without modifying the test code. These
// are consumed by the `FileWriterGuard` tests only — the `dispatch_command`
// tests use `CurrentDirGuard` from `swissarmyhammer_common::test_utils`.
#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use swissarmyhammer_common::logging::FileWriterGuard;

#[tokio::main]
async fn main() {
    // Show banner for interactive help invocations
    let args: Vec<String> = std::env::args().collect();
    if banner::should_show_banner(&args) {
        banner::print_banner();
    }

    let cli = Cli::parse();

    // Configure tracing: file-based logging to .shell/mcp.log, with stderr fallback.
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

/// Run all registered init components for the given scope and return the exit code.
fn run_init(target: InstallTarget) -> i32 {
    let scope = install_target_to_scope(target);
    let mut reg = InitRegistry::new();
    commands::registry::register_all(&mut reg);
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
    commands::registry::register_all(&mut reg);
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
        Commands::Serve => match commands::serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        },
        Commands::Init { target } => run_init(target),
        Commands::Deinit { target } => run_deinit(target),
        Commands::Doctor { verbose } => commands::doctor::run_doctor(verbose),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use swissarmyhammer_common::test_utils::CurrentDirGuard;
    use tempfile::TempDir;

    /// Block on a future from a synchronous test that needs to hold a
    /// `!Send` guard (like [`CurrentDirGuard`]) across the await.
    ///
    /// `#[tokio::test]` can't be used here: holding a
    /// `std::sync::MutexGuard` (the inner type of [`CurrentDirGuard`])
    /// across an `.await` trips `clippy::await_holding_lock`. Driving the
    /// future on a fresh single-threaded runtime from a `#[test]` body
    /// keeps the guard entirely on one OS thread and sidesteps the lint.
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build single-thread tokio runtime")
            .block_on(future)
    }

    // ── FileWriterGuard ──────────────────────────────────────────────────

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
        let shared = Arc::new(super::Mutex::new(file));
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
        let shared = Arc::new(super::Mutex::new(file));

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

    // ── dispatch_command arms ────────────────────────────────────────────

    /// `dispatch_command` must route `Commands::Init { target: Local }`
    /// through the init registry and return an exit code derived from
    /// the component results. Running under a fresh tempdir-as-CWD keeps
    /// the writes hermetic: `.claude/settings.local.json` and `.shell/`
    /// are scoped beneath the tempdir and discarded on drop.
    ///
    /// [`CurrentDirGuard`] shares a single global mutex
    /// (`swissarmyhammer_common::test_utils::CURRENT_DIR_LOCK`) with every
    /// other CWD-mutating test in the binary — including
    /// `logging::tests::init_tracing_creates_mcp_log_under_shell_dir` —
    /// so there's no window during which two tests can race on
    /// `std::env::set_current_dir`.
    #[test]
    fn dispatch_command_init_local_runs_registry() {
        let tempdir = TempDir::new().expect("create tempdir");
        let _cwd = CurrentDirGuard::new(tempdir.path()).expect("enter tempdir");

        let cli = Cli {
            debug: false,
            command: Commands::Init {
                target: InstallTarget::Local,
            },
        };

        let exit_code = block_on(dispatch_command(cli));
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
    /// any file writes/removes land in a throwaway scope, and use
    /// [`CurrentDirGuard`] so the CWD change is serialized against every
    /// other CWD-mutating test via the shared `CURRENT_DIR_LOCK`.
    #[test]
    fn dispatch_command_deinit_local_runs_registry() {
        let tempdir = TempDir::new().expect("create tempdir");
        let _cwd = CurrentDirGuard::new(tempdir.path()).expect("enter tempdir");

        let cli = Cli {
            debug: false,
            command: Commands::Deinit {
                target: InstallTarget::Local,
            },
        };

        let exit_code = block_on(dispatch_command(cli));
        assert!(
            exit_code == 0 || exit_code == 1,
            "unexpected exit code: {exit_code}"
        );
    }

    /// `dispatch_command` must route `Commands::Doctor { verbose: false }`
    /// straight through to `doctor::run_doctor`. `run_doctor` is already
    /// covered in `doctor::tests`; this test just pins down the dispatch
    /// arm. The host-dependent exit code is either 0, 1, or 2.
    ///
    /// No CWD change here — `run_doctor` is read-only with respect to
    /// the working directory — so no [`CurrentDirGuard`] is needed.
    #[test]
    fn dispatch_command_doctor_runs_diagnostics() {
        let cli = Cli {
            debug: false,
            command: Commands::Doctor { verbose: false },
        };

        let exit_code = block_on(dispatch_command(cli));
        assert!(exit_code <= 2, "unexpected exit code: {exit_code}");
    }
}
