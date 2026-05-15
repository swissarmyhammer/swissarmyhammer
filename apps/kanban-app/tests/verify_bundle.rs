//! Integration test for `build-support/verify-app-bundle.sh`, the script that
//! gates the release pipeline by proving a built `.app` bundle carries a
//! working `kanban` CLI.
//!
//! The release workflow (`.github/workflows/release-app.yml`, job
//! `build-macos`) shells out to this script against the freshly built bundle.
//! With `--require-cli` it asserts that `Contents/MacOS/kanban` exists, is
//! executable, and reports a non-empty version; `codesign` verification is
//! exercised separately and is skipped here via `--skip-signing` because the
//! mock bundles in this test are unsigned.
//!
//! The script lives in the repo-root `build-support/` directory; this crate
//! sits at `apps/kanban-app/`, two levels below the workspace root.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Absolute path to `build-support/verify-app-bundle.sh`, resolved from this
/// crate's manifest directory (`apps/kanban-app/`) by walking up to the repo
/// root.
fn verify_script() -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent() // apps/
        .and_then(Path::parent) // repo root
        .expect("apps/kanban-app must sit two levels below the workspace root");
    repo_root.join("build-support").join("verify-app-bundle.sh")
}

/// Run `verify-app-bundle.sh` with the given arguments and return its captured
/// output. Unlike `cask_gen.rs`'s helper this does not assert success — the
/// exit status is the subject under test.
fn run_verify(args: &[&str]) -> Output {
    let script = verify_script();
    assert!(
        script.exists(),
        "verify-app-bundle.sh should exist at {}",
        script.display()
    );

    Command::new("bash")
        .arg(&script)
        .args(args)
        .output()
        .expect("verify-app-bundle.sh should be invocable via bash")
}

/// Build a mock `.app` bundle directory tree under `parent`.
///
/// Always creates `Contents/MacOS/`. When `cli` is `Some`, writes a
/// `Contents/MacOS/kanban` script with the given executable mode whose
/// `--version` invocation prints `mock-kanban 9.9.9`.
///
/// Returns the path to the created `.app` directory.
fn make_mock_bundle(parent: &Path, cli: Option<u32>) -> PathBuf {
    let bundle = parent.join("Kanban.app");
    let macos = bundle.join("Contents").join("MacOS");
    std::fs::create_dir_all(&macos).expect("mock bundle Contents/MacOS must be creatable");

    if let Some(mode) = cli {
        let cli_path = macos.join("kanban");
        // A tiny shell stub: it prints a version on `--version`, mirroring
        // what the real sidecar CLI does.
        std::fs::write(
            &cli_path,
            "#!/usr/bin/env bash\n\
             if [ \"$1\" = \"--version\" ]; then echo \"mock-kanban 9.9.9\"; exit 0; fi\n\
             exit 0\n",
        )
        .expect("mock kanban CLI must be writable");
        set_mode(&cli_path, mode);
    }

    bundle
}

/// Set the Unix permission bits of `path`.
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .expect("setting mock CLI permissions must succeed");
}

/// On non-Unix platforms there is no executable permission bit; this is a
/// no-op so the helper compiles, though the verify script targets macOS.
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

/// A bundle whose `kanban` CLI is present, executable, and version-reporting
/// must pass `--require-cli` verification (signing skipped).
#[test]
fn require_cli_passes_for_working_bundle() {
    let tmp = tempfile::tempdir().expect("tempdir must be creatable");
    let bundle = make_mock_bundle(tmp.path(), Some(0o755));

    let output = run_verify(&[
        bundle.to_str().expect("bundle path is UTF-8"),
        "--require-cli",
        "--skip-signing",
    ]);

    assert!(
        output.status.success(),
        "verify-app-bundle.sh should exit 0 for a working bundle; \
         exit: {} stdout: {} stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// A bundle with no `Contents/MacOS/kanban` must fail `--require-cli`
/// verification with a descriptive error.
#[test]
fn require_cli_fails_when_cli_absent() {
    let tmp = tempfile::tempdir().expect("tempdir must be creatable");
    let bundle = make_mock_bundle(tmp.path(), None);

    let output = run_verify(&[
        bundle.to_str().expect("bundle path is UTF-8"),
        "--require-cli",
        "--skip-signing",
    ]);

    assert!(
        !output.status.success(),
        "verify-app-bundle.sh should exit non-zero when the CLI is absent",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("kanban"),
        "failure message should mention the missing `kanban` CLI; got: {stderr}",
    );
}

/// A bundle whose `kanban` exists but lacks the executable bit must fail
/// `--require-cli` verification.
#[test]
fn require_cli_fails_when_cli_not_executable() {
    let tmp = tempfile::tempdir().expect("tempdir must be creatable");
    // Mode 0o644 — readable/writable but not executable.
    let bundle = make_mock_bundle(tmp.path(), Some(0o644));

    let output = run_verify(&[
        bundle.to_str().expect("bundle path is UTF-8"),
        "--require-cli",
        "--skip-signing",
    ]);

    assert!(
        !output.status.success(),
        "verify-app-bundle.sh should exit non-zero when the CLI is not executable; \
         stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("executable"),
        "failure message should mention the missing executable bit; got: {stderr}",
    );
}

/// Without `--require-cli` a CLI-less bundle must still pass (signing
/// skipped) — this is the `mirdan` case, an app that ships no CLI.
#[test]
fn cli_less_bundle_passes_without_require_cli() {
    let tmp = tempfile::tempdir().expect("tempdir must be creatable");
    let bundle = make_mock_bundle(tmp.path(), None);

    let output = run_verify(&[
        bundle.to_str().expect("bundle path is UTF-8"),
        "--skip-signing",
    ]);

    assert!(
        output.status.success(),
        "verify-app-bundle.sh should exit 0 for a CLI-less bundle without \
         --require-cli; exit: {} stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

/// A non-existent bundle path must fail verification regardless of flags.
#[test]
fn missing_bundle_fails() {
    let tmp = tempfile::tempdir().expect("tempdir must be creatable");
    let missing = tmp.path().join("DoesNotExist.app");

    let output = run_verify(&[
        missing.to_str().expect("bundle path is UTF-8"),
        "--skip-signing",
    ]);

    assert!(
        !output.status.success(),
        "verify-app-bundle.sh should exit non-zero for a missing bundle",
    );
}
