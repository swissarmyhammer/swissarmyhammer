//! End-to-end integration tests for the `kanban` binary's lifecycle subcommands.
//!
//! These tests launch the compiled `kanban` binary as a child process and
//! assert on exit codes and captured output for the four lifecycle commands
//! wired into `src/main.rs` (`serve`, `init`, `deinit`, `doctor`). They
//! mirror the style of `shelltool-cli/tests/cli.rs`.
//!
//! The binary path is resolved via `env!("CARGO_BIN_EXE_kanban")`, which
//! Cargo (and nextest) populates automatically for integration tests that
//! sit alongside a `[[bin]]` target — no pre-build step is required.

use std::io::Write;
use std::process::{Command, Stdio};

/// Absolute path to the compiled `kanban` binary, injected by Cargo.
const KANBAN_BIN: &str = env!("CARGO_BIN_EXE_kanban");

/// `kanban --help` must exit successfully and list every lifecycle subcommand
/// wired into `main.rs`.
///
/// This pins the public CLI surface: if a subcommand is renamed or dropped,
/// this test fails loudly instead of silently shipping a broken help screen.
/// The help output also contains the schema-driven noun commands (task,
/// board, etc.) and the existing `open`/`merge` subcommands — asserting on
/// only the lifecycle four keeps the test focused and stable against
/// schema churn.
#[test]
fn help_lists_all_lifecycle_subcommands() {
    let output = Command::new(KANBAN_BIN)
        .arg("--help")
        .output()
        .expect("failed to launch kanban binary");

    assert!(
        output.status.success(),
        "kanban --help should exit 0, got {:?} (stderr: {})",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("help output must be UTF-8");

    for subcommand in ["serve", "init", "deinit", "doctor"] {
        assert!(
            stdout.contains(subcommand),
            "kanban --help output should mention `{subcommand}` subcommand; got:\n{stdout}",
        );
    }
}

/// `kanban --help` must continue to list the pre-existing `open` and
/// `merge` subcommands even after the lifecycle commands are wired in.
/// This guards against accidental regression if the builder sections are
/// reorganized later.
#[test]
fn help_retains_existing_open_and_merge() {
    let output = Command::new(KANBAN_BIN)
        .arg("--help")
        .output()
        .expect("failed to launch kanban binary");

    assert!(output.status.success(), "kanban --help should exit 0");

    let stdout = String::from_utf8(output.stdout).expect("help output must be UTF-8");
    for subcommand in ["open", "merge"] {
        assert!(
            stdout.contains(subcommand),
            "kanban --help output should still mention `{subcommand}` subcommand; got:\n{stdout}",
        );
    }
}

/// `kanban doctor` must run the diagnostic pipeline and return a valid
/// exit code.
///
/// `run_doctor` classifies results as 0 (all ok), 1 (warnings), or 2
/// (errors). The concrete value depends on the host (git repo present,
/// kanban on PATH, `.kanban/board.yaml` present, etc.), so the test only
/// asserts the code is one of the three documented values — which is
/// enough to prove the command wired through cleanly without panicking.
#[test]
fn doctor_exits_with_valid_code() {
    let output = Command::new(KANBAN_BIN)
        .arg("doctor")
        .output()
        .expect("failed to launch kanban binary");

    let code = output
        .status
        .code()
        .expect("kanban doctor should exit normally, not via signal");

    assert!(
        matches!(code, 0..=2),
        "kanban doctor exit code should be 0, 1, or 2, got {code} (stderr: {})",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// `kanban doctor --verbose` must accept the verbose flag and still
/// return a valid doctor exit code (0, 1, or 2).
///
/// Verbose mode only affects presentation — the exit classification is
/// identical to the non-verbose run — so we re-assert the same invariant
/// here. The important thing this test pins down is that `--verbose` is
/// a recognized argument; an unknown flag would exit 2 from clap with an
/// error message on stderr. To distinguish "doctor said error" from
/// "clap rejected the flag", we additionally require stderr *not* to
/// contain clap's unknown-argument marker.
#[test]
fn doctor_verbose_is_accepted() {
    let output = Command::new(KANBAN_BIN)
        .args(["doctor", "--verbose"])
        .output()
        .expect("failed to launch kanban binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized"),
        "kanban doctor --verbose should accept the flag; stderr was: {stderr}",
    );

    let code = output
        .status
        .code()
        .expect("kanban doctor --verbose should exit normally, not via signal");

    assert!(
        matches!(code, 0..=2),
        "kanban doctor --verbose exit code should be 0, 1, or 2, got {code} (stderr: {stderr})",
    );
}

/// `kanban doctor -v` (short form) must also be recognized. Pins the
/// short-flag mapping so renaming one without the other (long kept,
/// short dropped or vice versa) fails here.
#[test]
fn doctor_short_verbose_is_accepted() {
    let output = Command::new(KANBAN_BIN)
        .args(["doctor", "-v"])
        .output()
        .expect("failed to launch kanban binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized"),
        "kanban doctor -v should accept the flag; stderr was: {stderr}",
    );

    let code = output
        .status
        .code()
        .expect("kanban doctor -v should exit normally, not via signal");

    assert!(matches!(code, 0..=2), "unexpected exit code: {code}");
}

/// `kanban init --help` must accept and describe the optional positional
/// `target` argument. We only assert on the help output (not actual init
/// behavior) because running init in the test environment would mutate
/// the host's agent config files.
#[test]
fn init_help_documents_target_argument() {
    let output = Command::new(KANBAN_BIN)
        .args(["init", "--help"])
        .output()
        .expect("failed to launch kanban binary");

    assert!(
        output.status.success(),
        "kanban init --help should exit 0, got {:?}",
        output.status.code(),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("target") || stdout.contains("TARGET"),
        "kanban init --help should mention the target argument; got:\n{stdout}",
    );
    for value in ["project", "local", "user"] {
        assert!(
            stdout.contains(value),
            "kanban init --help should list allowed target value `{value}`; got:\n{stdout}",
        );
    }
}

/// `kanban deinit --help` must mirror `init --help` — same optional
/// positional `target` argument with the same allowed values. Init and
/// deinit are inverses of each other and their surfaces must stay in
/// lockstep.
#[test]
fn deinit_help_documents_target_argument() {
    let output = Command::new(KANBAN_BIN)
        .args(["deinit", "--help"])
        .output()
        .expect("failed to launch kanban binary");

    assert!(
        output.status.success(),
        "kanban deinit --help should exit 0, got {:?}",
        output.status.code(),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("target") || stdout.contains("TARGET"),
        "kanban deinit --help should mention the target argument; got:\n{stdout}",
    );
    for value in ["project", "local", "user"] {
        assert!(
            stdout.contains(value),
            "kanban deinit --help should list allowed target value `{value}`; got:\n{stdout}",
        );
    }
}

/// `kanban init foobar` (an invalid target value) must be rejected by
/// clap with a non-success exit code. This pins the `value_parser`
/// restriction so a future refactor that drops the whitelist fails
/// loudly.
#[test]
fn init_rejects_invalid_target_value() {
    let output = Command::new(KANBAN_BIN)
        .args(["init", "foobar"])
        .output()
        .expect("failed to launch kanban binary");

    assert!(
        !output.status.success(),
        "kanban init foobar should fail clap validation, got exit 0",
    );
}

/// `kanban serve` must exit 0 when stdin closes cleanly after a valid
/// MCP initialize + `notifications/initialized` handshake. This is the
/// clean-shutdown acceptance criterion from the card — an MCP client
/// that completes the handshake and then closes its pipe must leave the
/// server exiting successfully, not with a "connection closed"
/// diagnostic.
///
/// The handshake uses the protocol version the `rmcp` server reports in
/// `get_info` (`2024-11-05`) so the two sides agree and the
/// `initialized` notification is accepted.
#[test]
fn serve_exits_cleanly_on_stdin_eof_after_initialize() {
    let mut child = Command::new(KANBAN_BIN)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch kanban binary");

    let stdin = child.stdin.as_mut().expect("child stdin must be piped");
    // Send a well-formed MCP initialize + initialized notification, then
    // close stdin. The server must respond to initialize, accept the
    // notification, and shut down with exit code 0 when EOF arrives.
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05","capabilities":{{}},"clientInfo":{{"name":"test","version":"1"}}}}}}"#
    )
    .expect("write initialize request");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .expect("write initialized notification");

    // Dropping the child's stdin handle closes the pipe — the server
    // should see EOF and shut down cleanly.
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("wait for kanban serve to exit");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "kanban serve should exit 0 on clean stdin EOF after handshake; stderr: {stderr}",
    );
}
