//! Integration tests for the bundled-CLI auto-install module.
//!
//! The `cli_install` module is part of the `kanban-app` binary crate, which
//! has no library target. To exercise its pure functions directly, this test
//! file compiles the module source into the test binary via `#[path]` — the
//! same independent-compilation pattern `build.rs` files use across this
//! workspace for binary-crate modules.
//!
//! Scope: only the filesystem-symlink functions are covered here. The
//! privilege-escalation (`osascript`) branch lives behind `pick_target_dir`'s
//! writability result and is deliberately not unit-tested — see the comment on
//! `pick_target_dir` in the module source.

// The module's launch-time entry points (`run`, `spawn`) and the
// privilege-escalation helpers are exercised by the real binary, not by these
// pure-function tests, so they read as dead code in this standalone test
// compilation unit only. The binary build itself stays strict.
#[path = "../src/cli_install.rs"]
#[allow(dead_code)]
mod cli_install;

use cli_install::{already_installed, install_cli_symlink, resolve_bundled_cli, InstallOutcome};
use std::fs;
use std::path::{Path, PathBuf};

/// Create a fake bundled CLI binary inside a `Kanban.app/Contents/MacOS`
/// directory tree rooted at `root`, returning the path to the `kanban` file.
///
/// The `…/Kanban.app/Contents/MacOS/kanban` shape is what `already_installed`
/// recognises as a Kanban-bundle link target.
fn make_bundled_cli(root: &Path) -> PathBuf {
    let macos_dir = root.join("Kanban.app/Contents/MacOS");
    fs::create_dir_all(&macos_dir).expect("create bundle MacOS dir");
    let bundled = macos_dir.join("kanban");
    fs::write(&bundled, b"#!/bin/sh\necho kanban\n").expect("write bundled CLI");
    bundled
}

/// Resolve a symlink's literal target (one hop, not canonicalised).
fn link_target(link: &Path) -> PathBuf {
    fs::read_link(link).expect("path should be a symlink")
}

#[test]
fn install_cli_symlink_creates_link_in_empty_dir() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let target_dir = tempfile::tempdir().expect("temp target dir");
    let bundled = make_bundled_cli(bundle_root.path());

    let outcome = install_cli_symlink(&bundled, target_dir.path())
        .expect("install into empty dir should succeed");

    assert_eq!(outcome, InstallOutcome::Created);
    let link = target_dir.path().join("kanban");
    assert!(link.is_symlink(), "a `kanban` symlink should exist");
    assert_eq!(
        link_target(&link),
        bundled,
        "the symlink should point at the bundled CLI"
    );
}

#[test]
fn install_cli_symlink_is_a_noop_when_already_current() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let target_dir = tempfile::tempdir().expect("temp target dir");
    let bundled = make_bundled_cli(bundle_root.path());

    install_cli_symlink(&bundled, target_dir.path()).expect("first install");
    let outcome =
        install_cli_symlink(&bundled, target_dir.path()).expect("second install should succeed");

    assert_eq!(outcome, InstallOutcome::AlreadyCurrent);
    let link = target_dir.path().join("kanban");
    assert!(link.is_symlink(), "the `kanban` symlink should still exist");
    assert_eq!(
        link_target(&link),
        bundled,
        "the symlink target should be unchanged"
    );
}

#[test]
fn install_cli_symlink_repairs_a_stale_kanban_bundle_link() {
    let stale_root = tempfile::tempdir().expect("temp stale bundle root");
    let fresh_root = tempfile::tempdir().expect("temp fresh bundle root");
    let target_dir = tempfile::tempdir().expect("temp target dir");

    // A previous, now-removed Kanban.app — the link points into its bundle.
    let stale_bundled = make_bundled_cli(stale_root.path());
    let fresh_bundled = make_bundled_cli(fresh_root.path());

    let link = target_dir.path().join("kanban");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&stale_bundled, &link).expect("seed stale symlink");

    let outcome = install_cli_symlink(&fresh_bundled, target_dir.path())
        .expect("repair of a stale Kanban link should succeed");

    assert_eq!(outcome, InstallOutcome::Repaired);
    assert!(link.is_symlink(), "the `kanban` symlink should still exist");
    assert_eq!(
        link_target(&link),
        fresh_bundled,
        "the stale link should be repaired to the fresh bundle"
    );
}

#[test]
fn install_cli_symlink_leaves_a_non_kanban_real_file_intact() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let target_dir = tempfile::tempdir().expect("temp target dir");
    let bundled = make_bundled_cli(bundle_root.path());

    // A pre-existing real (non-symlink) `kanban` on PATH — e.g. a different
    // tool of the same name. It must never be overwritten.
    let foreign = target_dir.path().join("kanban");
    fs::write(&foreign, b"foreign binary contents").expect("seed foreign file");

    let outcome = install_cli_symlink(&bundled, target_dir.path())
        .expect("installing alongside a foreign file should not error");

    assert_eq!(outcome, InstallOutcome::Skipped);
    assert!(
        !foreign.is_symlink(),
        "the foreign file must remain a real file"
    );
    assert_eq!(
        fs::read(&foreign).expect("foreign file should still be readable"),
        b"foreign binary contents",
        "the foreign file contents must be untouched"
    );
}

#[test]
fn resolve_bundled_cli_returns_sibling_when_present() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let macos_dir = bundle_root.path().join("Kanban.app/Contents/MacOS");
    fs::create_dir_all(&macos_dir).expect("create bundle MacOS dir");
    let app_exe = macos_dir.join("kanban-app");
    fs::write(&app_exe, b"app binary").expect("write app exe");
    let bundled = macos_dir.join("kanban");
    fs::write(&bundled, b"cli binary").expect("write sibling CLI");

    assert_eq!(
        resolve_bundled_cli(&app_exe),
        Some(bundled),
        "the sibling `kanban` next to the running app should be resolved"
    );
}

#[test]
fn resolve_bundled_cli_returns_none_when_absent() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let macos_dir = bundle_root.path().join("Kanban.app/Contents/MacOS");
    fs::create_dir_all(&macos_dir).expect("create bundle MacOS dir");
    let app_exe = macos_dir.join("kanban-app");
    fs::write(&app_exe, b"app binary").expect("write app exe");
    // No sibling `kanban` written.

    assert_eq!(
        resolve_bundled_cli(&app_exe),
        None,
        "no sibling CLI means resolution returns None"
    );
}

#[test]
fn already_installed_is_true_for_a_kanban_bundle_link() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let path_dir = tempfile::tempdir().expect("temp PATH dir");
    let our_bundled = make_bundled_cli(bundle_root.path());

    // A second Kanban.app whose CLI is already linked onto PATH — e.g. the
    // Homebrew cask already made this link.
    let cask_root = tempfile::tempdir().expect("temp cask bundle root");
    let cask_bundled = make_bundled_cli(cask_root.path());
    let link = path_dir.path().join("kanban");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&cask_bundled, &link).expect("seed cask symlink");

    assert!(
        already_installed(path_dir.path(), &our_bundled),
        "a `kanban` link into any Kanban.app bundle counts as already installed"
    );
}

#[test]
fn already_installed_is_false_for_an_empty_dir() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let path_dir = tempfile::tempdir().expect("temp PATH dir");
    let bundled = make_bundled_cli(bundle_root.path());

    assert!(
        !already_installed(path_dir.path(), &bundled),
        "an empty PATH dir has no `kanban` link, so nothing is installed"
    );
}

#[test]
fn already_installed_is_false_for_a_non_kanban_link() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let path_dir = tempfile::tempdir().expect("temp PATH dir");
    let bundled = make_bundled_cli(bundle_root.path());

    // A `kanban` symlink that points somewhere unrelated — not a Kanban
    // bundle. This must NOT be treated as an existing install.
    let unrelated = bundle_root.path().join("some-other-tool");
    fs::write(&unrelated, b"unrelated").expect("write unrelated target");
    let link = path_dir.path().join("kanban");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&unrelated, &link).expect("seed unrelated symlink");

    assert!(
        !already_installed(path_dir.path(), &bundled),
        "a `kanban` link outside any Kanban bundle is not an existing install"
    );
}
