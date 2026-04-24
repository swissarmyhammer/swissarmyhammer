//! Headless integration tests for `UIState` self-loading.
//!
//! These tests must compile without any GUI dependency (`tauri`,
//! `kanban-app`, etc.) — the whole point of the refactor they exercise is
//! that `UIState` owns its own loading semantics, so CLI, MCP, and
//! headless test callers can construct a live `UIState` without pulling
//! in the desktop crate.
//!
//! The tests cover two paths that live entirely inside the Tier 0
//! `swissarmyhammer-commands` crate:
//!
//! 1. Load from an explicit fixture path — the happy path that CLI and
//!    MCP callers use when they already know where the config lives.
//! 2. Load when no file exists yet — every fresh install hits this
//!    branch; defaults must be seeded without surfacing an error.
//!
//! The XDG-aware entry point (`default_ui_state`) lives in
//! `swissarmyhammer-kanban` to keep `swissarmyhammer-commands` free of
//! `swissarmyhammer-directory` (Tier 0 purity). See
//! `swissarmyhammer-kanban/tests/default_ui_state.rs` for its coverage.

use std::fs;

use swissarmyhammer_commands::UIState;
use tempfile::TempDir;

/// Build a minimal but non-default YAML fixture covering both persisted
/// scalars (keymap_mode, most_recent_board_path) and a per-window state
/// entry. Kept small enough that every assertion below exercises a
/// different field.
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
fn load_from_fixture_path_matches_fixture_contents() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("ui-state.yaml");
    fs::write(&path, fixture_yaml()).unwrap();

    let state = UIState::load(&path);

    assert_eq!(state.keymap_mode(), "vim");
    assert_eq!(state.open_boards(), vec!["/tmp/board-a".to_string()]);
    assert_eq!(
        state.inspector_stack("main"),
        vec!["task:01ABCDEFGHJKMNPQRSTVWXYZ01".to_string()]
    );
    assert_eq!(state.active_view_id("main"), "board-view");
    assert_eq!(state.most_recent_board(), Some("/tmp/board-a".to_string()));
}

#[test]
fn load_with_no_file_returns_seeded_defaults() {
    let tmp = TempDir::new().unwrap();
    // Deliberately do NOT create the file — `UIState::load` must treat a
    // missing file as "fresh install" and seed defaults silently.
    let path = tmp.path().join("does-not-exist.yaml");
    assert!(!path.exists());

    let state = UIState::load(&path);

    // Defaults: "cua" keymap, empty window map, no MRU, no open boards.
    assert_eq!(state.keymap_mode(), "cua");
    assert!(state.open_boards().is_empty());
    assert!(state.recent_boards().is_empty());
    assert!(state.most_recent_board().is_none());
    assert!(state.inspector_stack("main").is_empty());
}
