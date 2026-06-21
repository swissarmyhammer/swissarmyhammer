//! Build script that bakes the git commit SHA into the crate at compile time.
//!
//! A running `sah` process keeps the code it was launched with even after the
//! on-disk binary is rebuilt. By embedding the commit SHA (with a `-dirty`
//! marker for an uncommitted working tree) as the `SAH_GIT_SHA` environment
//! variable, every process can log exactly which build it is running, giving us
//! ground truth instead of arguing about old-vs-new code.
//!
//! This uses only `std::process::Command` and adds no dependencies. If git is
//! unavailable or any command fails, it falls back to `SAH_GIT_SHA=unknown` so
//! it can never break the build.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let sha = git_sha().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=SAH_GIT_SHA={sha}");

    emit_rerun_triggers();
}

/// Compute the short SHA of `HEAD`, appending `-dirty` when the working tree
/// has uncommitted changes. Returns `None` if git is unavailable or any command
/// fails, so the caller can fall back to a sentinel value.
fn git_sha() -> Option<String> {
    let short_sha = run_git(&["rev-parse", "--short=12", "HEAD"])?;
    let short_sha = short_sha.trim();
    if short_sha.is_empty() {
        return None;
    }

    let dirty = run_git(&["status", "--porcelain"])
        .map(|status| !status.trim().is_empty())
        .unwrap_or(false);

    if dirty {
        Some(format!("{short_sha}-dirty"))
    } else {
        Some(short_sha.to_string())
    }
}

/// Run a git command and return its stdout as a `String`, or `None` if the
/// command could not be spawned or exited unsuccessfully.
fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Emit `cargo:rerun-if-changed` triggers so the SHA is recomputed whenever the
/// commit changes. Resolves the real git paths via `git rev-parse --git-path`
/// so this works in linked worktrees (where `.git` is a file, not a directory).
fn emit_rerun_triggers() {
    // Always re-run when this script changes.
    println!("cargo:rerun-if-changed=build.rs");

    // HEAD changes on every checkout/commit; watch the resolved HEAD file.
    if let Some(head) = git_path("HEAD") {
        watch_path(&head);
    }

    // The branch ref file is updated by `git commit` on the current branch, so
    // watch the file the symbolic HEAD points at (e.g. refs/heads/main).
    if let Some(branch_ref) = current_branch_ref() {
        if let Some(ref_path) = git_path(&branch_ref) {
            watch_path(&ref_path);
        }
    }
}

/// Resolve a path inside the git directory (e.g. `HEAD`, `refs/heads/main`) to
/// an absolute filesystem path via `git rev-parse --git-path`, which is
/// worktree-aware.
fn git_path(relative: &str) -> Option<PathBuf> {
    let raw = run_git(&["rev-parse", "--git-path", relative])?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// Return the symbolic ref name HEAD points at (e.g. `refs/heads/main`), or
/// `None` in a detached-HEAD state where there is no branch ref to watch.
fn current_branch_ref() -> Option<String> {
    let raw = run_git(&["symbolic-ref", "--quiet", "HEAD"])?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Emit a `cargo:rerun-if-changed` for a path if it exists. We avoid emitting
/// one for a ref that has never been created (e.g. an unborn branch with no
/// commits).
fn watch_path(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
