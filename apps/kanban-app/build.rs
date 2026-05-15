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
//! a missing-sidecar error. Running the staging script here makes the crate
//! self-sufficient for both paths. The script is idempotent.

use std::path::Path;
use std::process::Command;

fn main() {
    stage_cli_sidecar();
    tauri_build::build();
}

/// Builds and stages the `kanban` CLI sidecar by invoking
/// `scripts/stage-cli-sidecar.sh` for the current target triple.
///
/// The target triple is read from cargo's `TARGET` environment variable so
/// the staged file name matches what `tauri-build` expects, whether the build
/// is for the host or a cross-compilation target (e.g. CI's
/// `--target aarch64-apple-darwin`).
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

    // Re-run staging if the script itself changes.
    println!("cargo:rerun-if-changed={}", script.display());

    let target = std::env::var("TARGET").expect("TARGET is always set by cargo for build scripts");

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
