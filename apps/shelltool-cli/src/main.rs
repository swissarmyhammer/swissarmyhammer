//! shelltool CLI — Standalone MCP shell tool for AI coding agents.
//!
//! Commands:
//! - `shelltool serve`: Run MCP server over stdio, exposing the shell tool
//! - `shelltool init [target]`: Install shelltool into Claude Code settings
//! - `shelltool deinit [target]`: Remove shelltool from Claude Code settings
//! - `shelltool doctor`: Diagnose shelltool setup
//! - `shelltool completion <shell>`: Generate shell completion scripts
//! - `shelltool <noun> <verb> ...`: Run a shell operation (e.g.
//!   `shelltool command execute`, `processes list`, `history grep`,
//!   `lines get`, `process kill`), generated at runtime from the shell tool
//!   schema.
//!
//! Exit codes:
//! - 0: Success
//! - 1: Error

use clap::Command;
use swissarmyhammer_cli_completions::lifecycle;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;
use swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments;
use swissarmyhammer_tools::mcp::tool_registry::McpTool;
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;
use tracing::error;

/// The program name — used for the clap root command, completion target, and
/// completion dispatch. Sourced once so a rename can't desync the sites.
const PROGRAM: &str = "shelltool";

mod banner;
mod cli;
mod commands;
mod logging;

// Re-exports used by the in-file `tests` module so that `use super::*;` resolves
// `Arc`, `Mutex`, and `FileWriterGuard` without modifying the test code. These
// are consumed by the `FileWriterGuard` tests only.
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

    // The op subcommand tree is built in-process from the shell tool's FULL
    // schema (per-op `x-operation-schemas` + flat properties), not the slim
    // wire form — that's what the shared `cli_gen` generator consumes.
    let schema = ShellExecuteTool::new().schema_full();

    // `--debug` is a global flag declared on the lifecycle root; pull it off the
    // raw args before clap so tracing is configured before dispatch.
    let debug = args.iter().any(|a| a == "--debug" || a == "-d");
    logging::init_tracing(debug);

    let cmd = build_cli(&schema);
    let matches = cmd.get_matches();

    let exit_code = dispatch(&matches, &schema).await;
    std::process::exit(exit_code);
}

/// Build the clap command tree for `shelltool`.
///
/// Delegates the whole root assembly — root command + global `--debug` +
/// schema-driven `noun → verb` shell operation subcommands + the five lifecycle
/// subcommands (`serve`/`init`/`deinit`/`doctor`/`completion`) — to the shared
/// [`lifecycle::standard_op_cli`]. shelltool has no app-specific subcommands of
/// its own, so it returns the standard tree unchanged. The lifecycle surface
/// mirrors the static `cli.rs` definition consumed by `build.rs`.
fn build_cli(schema: &serde_json::Value) -> Command {
    lifecycle::standard_op_cli(
        PROGRAM,
        "Replaces Bash/exec with a searchable shell that saves tokens",
        schema,
    )
}

/// Return `true` if any `InitResult` has `Error` status.
fn any_init_error(results: &[swissarmyhammer_common::lifecycle::InitResult]) -> bool {
    results
        .iter()
        .any(|r| r.status == swissarmyhammer_common::lifecycle::InitStatus::Error)
}

/// Install shelltool for the given scope and return the exit code.
///
/// Runs the mirdan profile installer (registers the `shelltool` MCP server and
/// deploys the builtin `shell` skill) followed by the genuine tool-lifecycle
/// components (`Bash` deny + `.shell/config.yaml`). A single errored result from
/// either phase demotes the run to exit code 1.
fn run_init(scope: InitScope) -> i32 {
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

    if any_init_error(&results) {
        1
    } else {
        0
    }
}

/// Remove shelltool for the given scope and return the exit code.
///
/// Mirrors [`run_init`]: deinits the genuine tool-lifecycle components, then runs
/// the mirdan profile deinstaller (unregisters the MCP server and removes the
/// `shell` skill).
fn run_deinit(scope: InitScope) -> i32 {
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

    if any_init_error(&results) {
        1
    } else {
        0
    }
}

/// Route the matched CLI invocation to the correct handler and return an exit code.
///
/// The lifecycle subcommands (`serve`, `init`, `deinit`, `doctor`, `completion`)
/// are handled inline, mirroring the static `cli.rs` surface. Any other
/// subcommand is a schema-driven shell operation: it is converted to a
/// `{ "op": "verb noun", ...args }` map via [`extract_noun_verb_arguments`] and
/// dispatched to [`commands::ops::run_operation`].
///
/// Returns an exit code: 0 for success, 1 for error.
async fn dispatch(matches: &clap::ArgMatches, schema: &serde_json::Value) -> i32 {
    match matches.subcommand() {
        Some(("serve", _)) => match commands::serve::run_serve().await {
            Ok(()) => 0,
            Err(e) => {
                error!("Error: {}", e);
                1
            }
        },
        Some(("init", sub_m)) => run_init(lifecycle::target_scope(sub_m)),
        Some(("deinit", sub_m)) => run_deinit(lifecycle::target_scope(sub_m)),
        Some(("doctor", sub_m)) => commands::doctor::run_doctor(sub_m.get_flag("verbose")),
        Some(("completion", sub_m)) => lifecycle::run_completion(build_cli(schema), PROGRAM, sub_m),
        Some((noun, _)) => {
            // A non-lifecycle subcommand is a schema-driven shell operation.
            match extract_noun_verb_arguments(matches, schema) {
                Ok(arguments) => commands::ops::run_operation(arguments).await,
                Err(e) => {
                    error!("Error: {}", e);
                    error!("Run '{PROGRAM} {noun} --help' or '{PROGRAM} --help' for usage.");
                    1
                }
            }
        }
        None => {
            error!("No command specified. Run '{PROGRAM} --help' for usage information.");
            1
        }
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

    // ── dispatch arms ────────────────────────────────────────────────────

    /// Parse an argv slice through the runtime command tree built by
    /// [`build_cli`] from the shell tool's full schema.
    fn matches_from(argv: &[&str]) -> (serde_json::Value, clap::ArgMatches) {
        let schema = ShellExecuteTool::new().schema_full();
        let matches = build_cli(&schema)
            .try_get_matches_from(argv)
            .expect("argv should parse against the runtime command tree");
        (schema, matches)
    }

    /// `dispatch` must route `init local` through the init registry and create
    /// the tool-lifecycle artifact. We assert the observable side effect — the
    /// `.shell/config.yaml` the `ShellExecuteTool` lifecycle writes — rather than
    /// the host-variable exit code (the MCP-registration phase depends on
    /// detectable agents). Running under a fresh tempdir-as-CWD keeps the writes
    /// hermetic and discarded on drop.
    ///
    /// `#[serial_test::serial(cwd)]` joins the crate-wide `cwd` serialization
    /// group shared by every CWD-touching test in this crate.
    #[test]
    #[serial_test::serial(cwd)]
    fn dispatch_init_local_creates_shell_config() {
        let tempdir = TempDir::new().expect("create tempdir");
        let _cwd = CurrentDirGuard::new(tempdir.path()).expect("enter tempdir");

        let (schema, matches) = matches_from(&[PROGRAM, "init", "local"]);
        let _ = block_on(dispatch(&matches, &schema));

        let config = tempdir.path().join(".shell").join("config.yaml");
        assert!(
            config.exists(),
            "init local should create .shell/config.yaml under the CWD"
        );
    }

    /// `dispatch` must route `deinit local` through the deinit registry and
    /// remove the `.shell/` config directory the matching `init` created. We
    /// assert the observable side effect (the directory is gone afterward)
    /// rather than the host-variable exit code.
    #[test]
    #[serial_test::serial(cwd)]
    fn dispatch_deinit_local_removes_shell_config() {
        let tempdir = TempDir::new().expect("create tempdir");
        let _cwd = CurrentDirGuard::new(tempdir.path()).expect("enter tempdir");

        // Seed the artifact via init so deinit has something to remove.
        let (schema, init_m) = matches_from(&[PROGRAM, "init", "local"]);
        let _ = block_on(dispatch(&init_m, &schema));
        let shell_dir = tempdir.path().join(".shell");
        assert!(shell_dir.exists(), "precondition: init created .shell/");

        let (schema, deinit_m) = matches_from(&[PROGRAM, "deinit", "local"]);
        let _ = block_on(dispatch(&deinit_m, &schema));
        assert!(
            !shell_dir.exists(),
            "deinit local should remove the .shell/ config directory"
        );
    }

    /// `dispatch` must route `doctor` through `doctor::run_doctor`, returning
    /// the doctor's own exit code. We assert on the value `dispatch` itself
    /// returns (its observable contract), mirroring how the sibling `init`/
    /// `deinit` tests assert on the side effect dispatch produced.
    ///
    /// To make the assertion deterministic AND distinguish the doctor arm from
    /// the catch-all op-extraction arm, we seed a malformed `.shell/config.yaml`
    /// under a hermetic tempdir-as-CWD: the project-config health check then
    /// fails to parse, forcing the doctor verdict to exit code 2 (error). If the
    /// `doctor` arm were deleted, `"doctor"` would fall through to the
    /// schema-op arm, fail `extract_noun_verb_arguments`, and return 1 — so this
    /// assertion would fail, which is exactly what we want.
    #[test]
    #[serial_test::serial(cwd)]
    fn dispatch_doctor_returns_doctor_verdict() {
        let tempdir = TempDir::new().expect("create tempdir");
        let _cwd = CurrentDirGuard::new(tempdir.path()).expect("enter tempdir");

        // Seed a project config that fails to parse so the shell-tool health
        // check emits an Error, pinning the doctor verdict to exit code 2.
        let shell_dir = tempdir.path().join(".shell");
        std::fs::create_dir_all(&shell_dir).expect("create .shell dir");
        std::fs::write(shell_dir.join("config.yaml"), "deny: [unclosed")
            .expect("write malformed config");

        let (schema, matches) = matches_from(&[PROGRAM, "doctor"]);
        let exit_code = block_on(dispatch(&matches, &schema));

        assert_eq!(
            exit_code, 2,
            "dispatch should route to the doctor and return its error-verdict \
             exit code (2) for a malformed project config"
        );
    }

    /// `dispatch` must route a schema-driven shell op (`list processes`) through
    /// [`extract_noun_verb_arguments`] into [`commands::ops::run_operation`] and
    /// return success.
    #[test]
    fn dispatch_shell_op_reaches_tool() {
        let (schema, matches) = matches_from(&[PROGRAM, "processes", "list"]);
        let exit_code = block_on(dispatch(&matches, &schema));
        assert_eq!(
            exit_code, 0,
            "list processes should reach the tool and succeed"
        );
    }
}
