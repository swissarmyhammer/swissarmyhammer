//! Integration tests for the bundled-CLI auto-install module.
//!
//! The `cli_install` module is part of the `kanban-app` binary crate, which
//! has no library target. To exercise its pure functions directly, this test
//! file compiles the module source into the test binary via `#[path]` — the
//! same independent-compilation pattern `build.rs` files use across this
//! workspace for binary-crate modules.
//!
//! Scope: the filesystem-symlink functions and the pure AppleScript-source
//! builder are covered here. The privilege-escalation step itself —
//! constructing an `NSAppleScript` and calling `executeAndReturnError` — lives
//! behind `pick_target_dir`'s writability result and is deliberately not
//! unit-tested; see the comment on `pick_target_dir` in the module source.
//! Everything testable about the escalation is isolated in
//! `build_install_applescript`, which is platform-neutral and exercised below.

// The module's launch-time entry points (`run`, `spawn`) and the
// privilege-escalation helpers are exercised by the real binary, not by these
// pure-function tests, so they read as dead code in this standalone test
// compilation unit only. The binary build itself stays strict.
#[path = "../src/cli_install.rs"]
#[allow(dead_code)]
mod cli_install;

use cli_install::{
    already_installed, build_install_applescript, install_cli_symlink, resolve_bundled_cli,
    InstallOutcome,
};
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
fn already_installed_is_true_for_a_link_to_the_running_apps_cli() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let path_dir = tempfile::tempdir().expect("temp PATH dir");
    let bundled = make_bundled_cli(bundle_root.path());

    // The `kanban` link on PATH points at exactly the running app's bundled
    // CLI — this is our symlink, so nothing needs to be done.
    let link = path_dir.path().join("kanban");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&bundled, &link).expect("seed our symlink");

    assert!(
        already_installed(path_dir.path(), &bundled),
        "a `kanban` link pointing exactly at the running app's CLI counts as installed"
    );
}

#[test]
fn already_installed_is_false_for_a_link_into_a_different_kanban_bundle() {
    let path_dir = tempfile::tempdir().expect("temp PATH dir");

    // The running app's bundled CLI.
    let our_root = tempfile::tempdir().expect("temp our bundle root");
    let our_bundled = make_bundled_cli(our_root.path());

    // A `kanban` link into a *different* Kanban.app — same bundle shape, but a
    // stale/moved/replaced bundle. This is NOT our symlink, so the install
    // must proceed to repair it.
    let other_root = tempfile::tempdir().expect("temp other bundle root");
    let other_bundled = make_bundled_cli(other_root.path());
    let link = path_dir.path().join("kanban");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&other_bundled, &link).expect("seed stale-bundle symlink");

    assert!(
        !already_installed(path_dir.path(), &our_bundled),
        "a `kanban` link into a different Kanban bundle is not our install"
    );
}

#[test]
fn already_installed_is_false_for_a_real_non_symlink_file() {
    let bundle_root = tempfile::tempdir().expect("temp bundle root");
    let path_dir = tempfile::tempdir().expect("temp PATH dir");
    let bundled = make_bundled_cli(bundle_root.path());

    // A real (non-symlink) `kanban` file on PATH — e.g. a `cargo install`-ed
    // `kanban` binary. It is not our symlink, so it must not be mistaken for
    // an existing install.
    let real_file = path_dir.path().join("kanban");
    fs::write(&real_file, b"cargo-installed kanban binary").expect("seed real file");

    assert!(
        !already_installed(path_dir.path(), &bundled),
        "a real (non-symlink) `kanban` file is not our managed symlink install"
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

#[test]
fn build_install_applescript_explains_the_cli_tool_purpose() {
    let bundled = PathBuf::from("/Applications/Kanban.app/Contents/MacOS/kanban");
    let link = PathBuf::from("/usr/local/bin/kanban");

    let script = build_install_applescript(&bundled, &link);

    // The explanatory dialog must name the `kanban` command-line tool so the
    // user understands what the upcoming password prompt is for.
    assert!(
        script.contains("display dialog"),
        "the script should open an explanatory dialog before the password prompt:\n{script}"
    );
    assert!(
        script.contains("command-line tool"),
        "the dialog text should identify the kanban command-line tool:\n{script}"
    );
    assert!(
        script.contains("kanban"),
        "the dialog text should name the `kanban` tool:\n{script}"
    );
}

#[test]
fn build_install_applescript_offers_an_explicit_install_choice() {
    let bundled = PathBuf::from("/Applications/Kanban.app/Contents/MacOS/kanban");
    let link = PathBuf::from("/usr/local/bin/kanban");

    let script = build_install_applescript(&bundled, &link);

    // `Install` is the default button and `Not Now` the cancel button — a
    // cancel raises AppleScript error -128 and short-circuits before the
    // privileged step, exactly like a declined password prompt.
    assert!(
        script.contains("default button \"Install\""),
        "`Install` should be the default button:\n{script}"
    );
    assert!(
        script.contains("cancel button \"Not Now\""),
        "`Not Now` should be the cancel button:\n{script}"
    );
}

#[test]
fn build_install_applescript_links_bundled_to_link_with_admin_privileges() {
    let bundled = PathBuf::from("/Applications/Kanban.app/Contents/MacOS/kanban");
    let link = PathBuf::from("/usr/local/bin/kanban");

    let script = build_install_applescript(&bundled, &link);

    assert!(
        script.contains("with administrator privileges"),
        "the privileged step must request administrator privileges:\n{script}"
    );
    assert!(
        script.contains(
            "ln -sf '/Applications/Kanban.app/Contents/MacOS/kanban' '/usr/local/bin/kanban'"
        ),
        "the script should `ln -sf` the bundled CLI to the link path:\n{script}"
    );
}

#[test]
fn build_install_applescript_escapes_quotes_and_backslashes_in_paths() {
    // A path with both a double quote and a backslash — every such character
    // must be escaped for the AppleScript string literal, never emitted raw.
    let bundled = PathBuf::from("/Applications/Weird\"Kan\\ban.app/Contents/MacOS/kanban");
    let link = PathBuf::from("/usr/local/bin/kanban");

    let script = build_install_applescript(&bundled, &link);

    // The raw quote/backslash sequence must not appear unescaped inside the
    // `do shell script` string literal.
    assert!(
        !script.contains("Weird\"Kan\\ban.app"),
        "raw unescaped quote/backslash path must not appear in the script:\n{script}"
    );
    // Each `"` becomes `\"` and each `\` becomes `\\` in the AppleScript literal.
    assert!(
        script.contains("Weird\\\"Kan\\\\ban.app"),
        "quote and backslash in the path must be AppleScript-escaped:\n{script}"
    );
}
