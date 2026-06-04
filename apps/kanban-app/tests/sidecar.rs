//! Integration test asserting that `scripts/stage-cli-sidecar.sh` builds the
//! standalone `kanban` CLI and stages it where Tauri's `externalBin` mechanism
//! expects to find it.
//!
//! Tauri v2 resolves a sidecar declared as `binaries/kanban` to
//! `binaries/kanban-<target-triple>` and copies it into the app bundle as
//! `Kanban.app/Contents/MacOS/kanban`. This test exercises the staging script
//! end-to-end: it runs the script for the host triple, then verifies the
//! produced binary exists, is executable, and runs.
//!
//! Modeled on `apps/kanban-cli/tests/build_artifacts.rs`: paths are resolved
//! relative to `CARGO_MANIFEST_DIR` (the `apps/kanban-app/` crate root).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The `apps/kanban-app/` crate root, derived from the manifest directory.
fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The host target triple, as reported by `rustc -vV`'s `host:` line.
///
/// This matches the triple `stage-cli-sidecar.sh` derives when invoked
/// without a `--target` argument.
fn host_triple() -> String {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc must be on PATH to determine the host triple");
    let stdout = String::from_utf8(output.stdout).expect("rustc -vV output is UTF-8");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc -vV output must contain a `host:` line")
        .trim()
        .to_string()
}

#[test]
fn stage_cli_sidecar_produces_runnable_kanban_binary() {
    let app_root = app_root();
    let script = app_root.join("scripts/stage-cli-sidecar.sh");
    assert!(
        script.exists(),
        "staging script should exist at {}",
        script.display()
    );

    // Run the staging script for the host triple (no --target argument).
    //
    // Build in the `dev` profile, not the default `release`. This test asserts
    // the staging *contract* — the script places a runnable binary at the
    // triple-suffixed path Tauri resolves, with the executable bit set — which
    // is identical across profiles. A release build of the whole kanban-cli
    // dependency tree took ~80s and dominated the suite; the dev build reuses
    // the workspace's already-compiled debug artifacts. CI's real
    // `cargo tauri build` still uses the release profile via before-build.sh.
    let status = Command::new("bash")
        .arg(&script)
        .arg("--profile")
        .arg("dev")
        .current_dir(&app_root)
        .status()
        .expect("stage-cli-sidecar.sh should be invocable via bash");
    assert!(
        status.success(),
        "stage-cli-sidecar.sh exited with failure: {status}"
    );

    // Tauri's externalBin = ["binaries/kanban"] resolves to this path.
    let triple = host_triple();
    let staged = app_root.join("binaries").join(format!("kanban-{triple}"));
    assert!(
        staged.exists(),
        "staged sidecar should exist at {}",
        staged.display()
    );

    // The staged binary must carry the executable bit so Tauri can run it
    // and so it functions after being copied into the bundle.
    assert!(
        is_executable(&staged),
        "staged sidecar {} should have the executable bit set",
        staged.display()
    );

    // Running the staged binary with `--version` must exit 0 and print a
    // non-empty version string -- proving it is a real, working CLI.
    let output = Command::new(&staged)
        .arg("--version")
        .output()
        .expect("staged sidecar should be executable");
    assert!(
        output.status.success(),
        "`kanban --version` exited with failure: {}",
        output.status
    );
    let version = String::from_utf8(output.stdout).expect("`--version` output is UTF-8");
    assert!(
        !version.trim().is_empty(),
        "`kanban --version` printed an empty version string"
    );
}

/// Returns whether `path` has at least one executable permission bit set.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Non-Unix platforms have no executable permission bit; treat existence as
/// sufficient since Tauri keys executability off the `.exe` extension there.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.exists()
}
