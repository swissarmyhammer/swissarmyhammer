//! Integration tests for `swissarmyhammer_kanban::focus::resolve_focused_column`.
//!
//! This is the Rust replacement for the React `resolveFocusedColumnId`
//! helper that used to live in `kanban-app/ui/src/components/board-view.tsx`.
//! Per PR #40 review: column resolution is business logic, not presentation —
//! it belongs in a headless, testable Rust fn rather than in a React
//! component that requires jsdom / mock stores to exercise.
//!
//! The tests cover every branch of the resolver:
//! 1. `column:<id>` moniker → that id.
//! 2. `task:<tid>` moniker with a matching task→column entry → the column.
//! 3. Scope chain with no column/task moniker → `None` (backend falls back
//!    to the lowest-order column at the `AddEntity` layer).
//! 4. `task:<tid>` moniker whose id is not in the task→column map → `None`
//!    (commit to the lookup; do NOT silently fall through to an outer
//!    moniker or to a default column).
//!
//! Integration-level so they exercise the public API surface the rest of
//! the crate and downstream callers (kanban-app dispatch) rely on.

use std::collections::HashMap;
use swissarmyhammer_kanban::focus::resolve_focused_column;
use swissarmyhammer_kanban::{ColumnId, TaskId};

/// `column:<id>` in the innermost scope position resolves directly to that id
/// without consulting the task→column map.
#[test]
fn column_moniker_resolves_directly() {
    let scope = vec!["column:doing".to_string(), "window:main".to_string()];
    let map = HashMap::<TaskId, ColumnId>::new();

    assert_eq!(
        resolve_focused_column(&scope, &map),
        Some(ColumnId::from_string("doing"))
    );
}

/// `task:<tid>` with a matching entry in the task→column map resolves to
/// the task's home column.
#[test]
fn task_moniker_resolves_via_map() {
    let mut map = HashMap::new();
    map.insert(TaskId::from_string("01ABC"), ColumnId::from_string("doing"));

    let scope = vec![
        "task:01ABC".to_string(),
        "column:todo".to_string(),
        "window:main".to_string(),
    ];

    // The task's home column wins over the surrounding column scope — we
    // commit to the focused entity's column, not the board-level column
    // scope that happens to be in the chain.
    assert_eq!(
        resolve_focused_column(&scope, &map),
        Some(ColumnId::from_string("doing"))
    );
}

/// A scope chain whose innermost position is neither a column nor a task
/// moniker (and that contains no column/task monikers further in) returns
/// `None`, so the caller can fall back to the lowest-order column.
#[test]
fn neither_column_nor_task_returns_none() {
    let scope = vec!["window:main".to_string()];
    let map = HashMap::<TaskId, ColumnId>::new();

    assert_eq!(resolve_focused_column(&scope, &map), None);
}

/// An empty scope chain returns `None` — same contract as the "nothing
/// focused" branch of the React helper we're replacing.
#[test]
fn empty_scope_returns_none() {
    let scope: Vec<String> = vec![];
    let map = HashMap::<TaskId, ColumnId>::new();

    assert_eq!(resolve_focused_column(&scope, &map), None);
}

/// `task:<tid>` whose id is not in the task→column map returns `None`.
/// The resolver commits to the task lookup on first match — it must not
/// silently continue walking the chain and pick up some outer `column:*`
/// moniker, and it must not fall back to a default column. The caller
/// (AddEntity) sees `None` and defaults to the lowest-order column.
#[test]
fn task_moniker_missing_from_map_returns_none() {
    let map = HashMap::<TaskId, ColumnId>::new();
    let scope = vec![
        "task:unknown".to_string(),
        "column:todo".to_string(),
        "window:main".to_string(),
    ];

    assert_eq!(
        resolve_focused_column(&scope, &map),
        None,
        "unresolved task moniker must not silently fall through to an outer column"
    );
}

/// Innermost-first order is load-bearing: a focused `column:doing` in the
/// innermost position wins over an outer `column:todo`. Two column scopes
/// in the chain is a realistic case — the inner board column vs an outer
/// window-level scope — and the focused (innermost) one must win.
#[test]
fn innermost_column_wins_over_outer() {
    let scope = vec![
        "column:doing".to_string(),
        "column:todo".to_string(),
        "window:main".to_string(),
    ];
    let map = HashMap::<TaskId, ColumnId>::new();

    assert_eq!(
        resolve_focused_column(&scope, &map),
        Some(ColumnId::from_string("doing"))
    );
}

/// Non-column / non-task monikers at the innermost position (e.g. a
/// `field:*` scope pushed by an inline text-editor) do not block the
/// resolver from picking up a column/task moniker further in. This lets
/// "add a task while editing a task's title" still land correctly.
#[test]
fn skips_non_column_non_task_monikers() {
    let mut map = HashMap::new();
    map.insert(TaskId::from_string("01ABC"), ColumnId::from_string("doing"));

    let scope = vec![
        "field:task:01ABC.title".to_string(),
        "task:01ABC".to_string(),
        "column:todo".to_string(),
        "window:main".to_string(),
    ];

    assert_eq!(
        resolve_focused_column(&scope, &map),
        Some(ColumnId::from_string("doing"))
    );
}
