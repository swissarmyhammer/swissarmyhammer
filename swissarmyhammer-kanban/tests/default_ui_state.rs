//! Integration test for `swissarmyhammer_kanban::default_ui_state`.
//!
//! The helper is the XDG-aware entry point consumers (GUI, CLI, MCP)
//! use at startup: it resolves
//! `$XDG_CONFIG_HOME/sah/<app_subdir>/ui-state.yaml` and delegates the
//! file read to `UIState::load`. This test seeds a fixture at the exact
//! XDG-resolved path and asserts the helper picks it up, exercising
//! path composition end-to-end.
//!
//! Env-var mutation goes through `serial_test::serial` and
//! save/restore, matching the established pattern in
//! `swissarmyhammer-directory/src/file_loader.rs`. Without the
//! restore, a developer running this test with `XDG_CONFIG_HOME`
//! already set would silently lose it on cleanup.

use std::fs;

use serial_test::serial;
use swissarmyhammer_kanban::default_ui_state;
use tempfile::TempDir;

/// Minimal non-default YAML fixture. Mirrors the shape used in
/// `swissarmyhammer-commands/tests/ui_state_load.rs` so the two test
/// suites assert the same loader behavior from opposite entry points.
fn fixture_yaml() -> &'static str {
    "\
keymap_mode: vim
open_boards:
  - /tmp/board-a
windows:
  main:
    board_path: /tmp/board-a
    inspector_stack:
      - task:01ABCDEFGHJKMNPQRSTVWXYZ01
    active_view_id: board-view
    active_perspective_id: ''
    x: 100
    y: 200
    width: 1200
    height: 800
    maximized: false
recent_boards: []
most_recent_board_path: /tmp/board-a
"
}

#[test]
#[serial]
fn default_ui_state_reads_under_xdg_config_home() {
    // Point `$XDG_CONFIG_HOME` at a temp dir so we exercise the real
    // XDG resolution without touching the developer's actual config.
    let tmp = TempDir::new().unwrap();
    let app_subdir = "kanban-app-test-load";

    // Seed the file at the exact path `default_ui_state` should resolve
    // to: $XDG_CONFIG_HOME/sah/<app_subdir>/ui-state.yaml
    let target_dir = tmp.path().join("sah").join(app_subdir);
    fs::create_dir_all(&target_dir).unwrap();
    let target_path = target_dir.join("ui-state.yaml");
    fs::write(&target_path, fixture_yaml()).unwrap();

    // Capture the developer's prior value so cleanup restores it
    // instead of unconditionally removing the var. `#[serial]` keeps
    // other env-mutating tests from racing on the same variable.
    let original = std::env::var("XDG_CONFIG_HOME").ok();
    // SAFETY: `std::env::set_var` is unsafe in edition 2024 / Rust 1.80+
    // because concurrent `getenv` reads from other threads are UB.
    // `#[serial]` serializes this test against every other test in the
    // binary tagged the same way, so no sibling test can call into libc
    // `getenv` while we're here. The value we write is a path produced
    // by `TempDir` and dropped at the end of the test.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
    }

    let state = default_ui_state(app_subdir);

    // Restore the env var before assertions so an assertion failure
    // never leaves the developer's shell dirty. Mirrors the
    // save/restore pattern in
    // `swissarmyhammer-directory/src/file_loader.rs`.
    // SAFETY: same reasoning as the `set_var` above — still inside the
    // `#[serial]`-protected region.
    unsafe {
        match original {
            Some(ref v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert_eq!(state.keymap_mode(), "vim");
    assert_eq!(state.most_recent_board(), Some("/tmp/board-a".to_string()));
}

/// When the XDG file doesn't exist, `default_ui_state` must silently
/// seed defaults instead of surfacing an error. Every fresh install
/// hits this branch.
#[test]
#[serial]
fn default_ui_state_without_file_returns_defaults() {
    let tmp = TempDir::new().unwrap();
    let app_subdir = "kanban-app-test-missing";

    // Deliberately do NOT create the target file under the XDG root.
    let original = std::env::var("XDG_CONFIG_HOME").ok();
    // SAFETY: see `default_ui_state_reads_under_xdg_config_home` for
    // the full rationale — `#[serial]` rules out concurrent `getenv`.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
    }

    let state = default_ui_state(app_subdir);

    // SAFETY: same reasoning as the matching `set_var` above.
    unsafe {
        match original {
            Some(ref v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert_eq!(state.keymap_mode(), "cua");
    assert!(state.open_boards().is_empty());
    assert!(state.most_recent_board().is_none());
}
