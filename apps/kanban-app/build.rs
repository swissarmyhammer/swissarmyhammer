//! Build script for `kanban-app`.
//!
//! In addition to the standard `tauri_build::build()` call, this stages the
//! `kanban` CLI sidecar binary that `tauri.conf.json` declares via
//! `bundle.externalBin = ["binaries/kanban"]`.
//!
//! `tauri-build` validates that every `externalBin` path resolves to a
//! `binaries/kanban-<target-triple>` file at compile time. During a real
//! `cargo tauri build`, the `beforeBuildCommand` wrapper stages that file
//! first. But a plain `cargo build` or `cargo test -p kanban-app` skips the
//! Tauri CLI entirely, so without this step the build script would fail with
//! a missing-sidecar error. Staging here, only when the file is absent, makes
//! the crate self-sufficient for those paths.
//!
//! Crucially, staging shells out to `cargo build -p kanban-cli`. If that ran
//! while an outer `cargo build`/`cargo tauri build` already held the lock on
//! the SAME `target/<triple>/release` artifact directory, the inner cargo
//! would block forever on "waiting for file lock on artifact directory" while
//! the outer waits for this build script to return -- a deadlock. That is
//! exactly what hung the release CI for 6+ hours: `cargo tauri build --target
//! aarch64-apple-darwin` ran the `beforeBuildCommand` staging step (which
//! produced the sidecar), then compiled `kanban-app`, whose build script
//! re-ran staging and the nested cargo deadlocked against the outer build.
//! Skipping when the sidecar already exists removes the nested cargo from that
//! path entirely; the `beforeBuildCommand` step is what keeps the bundled
//! sidecar fresh, and tauri-build only needs the file to exist for validation.

use std::path::Path;
use std::process::Command;

fn main() {
    stage_cli_sidecar();
    tauri_build::build();
}

/// Stages the `kanban` CLI sidecar for the current target triple, but only if
/// it is not already present.
///
/// The target triple is read from cargo's `TARGET` environment variable so
/// the staged file name matches what `tauri-build` expects, whether the build
/// is for the host or a cross-compilation target (e.g. CI's
/// `--target aarch64-apple-darwin`).
///
/// When the sidecar is missing, this invokes `scripts/stage-cli-sidecar.sh`,
/// which builds `kanban-cli` via a nested `cargo` call. It deliberately does
/// NOT run when the file already exists: under `cargo tauri build` the
/// `beforeBuildCommand` wrapper has already staged a fresh sidecar before this
/// build script runs, and re-running the nested cargo there deadlocks on the
/// outer build's artifact-directory lock (see module docs).
///
/// Panics if the staging script is missing or exits non-zero, since the
/// subsequent `tauri_build::build()` call would otherwise fail with a less
/// actionable missing-sidecar error.
fn stage_cli_sidecar() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    let script = Path::new(&manifest_dir)
        .join("scripts")
        .join("stage-cli-sidecar.sh");

    let target = std::env::var("TARGET").expect("TARGET is always set by cargo for build scripts");

    // If the sidecar is already staged, do nothing -- re-running the staging
    // script would shell out to `cargo build` and deadlock against an outer
    // build holding the same artifact-directory lock (see module docs).
    let staged = Path::new(&manifest_dir)
        .join("binaries")
        .join(format!("kanban-{target}"));
    if staged.exists() {
        // Re-run this build script if the staged sidecar is replaced (e.g. by
        // the beforeBuildCommand step on the next build).
        println!("cargo:rerun-if-changed={}", staged.display());
        return;
    }

    // Re-run staging if the script itself changes.
    println!("cargo:rerun-if-changed={}", script.display());

    let status = Command::new("bash")
        .arg(&script)
        .arg("--target")
        .arg(&target)
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to invoke scripts/stage-cli-sidecar.sh");

    assert!(
        status.success(),
        "stage-cli-sidecar.sh exited with failure: {status}"
    );
}
