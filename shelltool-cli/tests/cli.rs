//! End-to-end integration tests for the `shelltool` binary.
//!
//! These tests launch the compiled `shelltool` binary as a child process and
//! assert on exit codes and captured output. They mirror the style of the
//! integration tests in the sibling CLIs (e.g. `kanban-cli/tests/merge_e2e.rs`).
//!
//! The binary path is resolved via `env!("CARGO_BIN_EXE_shelltool")`, which
//! Cargo (and nextest) populates automatically for integration tests that
//! sit alongside a `[[bin]]` target — no pre-build step is required.

use std::process::Command;

/// Absolute path to the compiled `shelltool` binary, injected by Cargo.
const SHELLTOOL_BIN: &str = env!("CARGO_BIN_EXE_shelltool");

/// `shelltool --help` must exit successfully and list every top-level
/// subcommand declared in `cli::Commands`.
///
/// This pins the public CLI surface: if a subcommand is renamed or dropped,
/// this test fails loudly instead of silently shipping a broken help screen.
#[test]
fn help_lists_all_subcommands() {
    let output = Command::new(SHELLTOOL_BIN)
        .arg("--help")
        .output()
        .expect("failed to launch shelltool binary");

    assert!(
        output.status.success(),
        "shelltool --help should exit 0, got {:?} (stderr: {})",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("help output must be UTF-8");

    for subcommand in ["serve", "init", "deinit", "doctor"] {
        assert!(
            stdout.contains(subcommand),
            "shelltool --help output should mention `{subcommand}` subcommand; got:\n{stdout}",
        );
    }
}

/// `shelltool doctor` must run the diagnostic pipeline and return a valid
/// exit code.
///
/// `run_doctor` classifies results as 0 (all ok), 1 (warnings), or 2
/// (errors). The concrete value depends on the host (git repo present,
/// shelltool on PATH, etc.), so the test only asserts the code is one of
/// the three documented values — which is enough to prove the command
/// wired through cleanly without panicking.
#[test]
fn doctor_exits_with_valid_code() {
    let output = Command::new(SHELLTOOL_BIN)
        .arg("doctor")
        .output()
        .expect("failed to launch shelltool binary");

    let code = output
        .status
        .code()
        .expect("shelltool doctor should exit normally, not via signal");

    assert!(
        matches!(code, 0..=2),
        "shelltool doctor exit code should be 0, 1, or 2, got {code} (stderr: {})",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// `shelltool doctor --verbose` must accept the verbose flag and still
/// return a valid doctor exit code (0, 1, or 2).
///
/// Verbose mode only affects presentation — the exit classification is
/// identical to the non-verbose run — so we re-assert the same invariant
/// here. The important thing this test pins down is that `--verbose` is
/// a recognized argument; an unknown flag would exit 2 from clap with an
/// error message on stderr, which this assertion still tolerates. To
/// distinguish "doctor said error" from "clap rejected the flag", we
/// additionally require stderr *not* to contain clap's unknown-argument
/// marker.
#[test]
fn doctor_verbose_is_accepted() {
    let output = Command::new(SHELLTOOL_BIN)
        .args(["doctor", "--verbose"])
        .output()
        .expect("failed to launch shelltool binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized"),
        "shelltool doctor --verbose should accept the flag; stderr was: {stderr}",
    );

    let code = output
        .status
        .code()
        .expect("shelltool doctor --verbose should exit normally, not via signal");

    assert!(
        matches!(code, 0..=2),
        "shelltool doctor --verbose exit code should be 0, 1, or 2, got {code} (stderr: {stderr})",
    );
}
