//! Focused-column resolution from a scope chain.
//!
//! This is the Rust implementation of the resolver that used to live in
//! `kanban-app/ui/src/components/board-view.tsx` as `resolveFocusedColumnId`.
//! Per PR #40 review feedback, column resolution is business logic, not
//! presentation — it belongs in a headless, testable Rust fn that any
//! consumer (GUI, CLI, MCP) can reuse without spinning up jsdom / React /
//! mock stores.
//!
//! The resolver is consumed by [`crate::entity::add::AddEntity`] at dispatch
//! time: when no explicit `column` override is present in the arg bag, the
//! dispatcher walks the scope chain with this resolver and uses the result
//! as the column override. `None` means "no focused column context" and
//! falls through to the default placement path (lowest-order column).

use crate::types::{ColumnId, TaskId};
use std::collections::HashMap;

/// Resolve the focused column id implied by a scope chain.
///
/// Walks the chain innermost-first (the frontend populates index 0 with the
/// focused entity and subsequent indices walk outward through nested
/// scopes). On the first moniker that matches `column:` or `task:`, the
/// resolver commits:
///
/// - `column:<id>` → `Some(ColumnId(<id>))`.
/// - `task:<tid>` → `task_to_column.get(<tid>)` (cloned) — `Some(_)` if the
///   task is known to the caller, `None` otherwise. The resolver does NOT
///   fall through to a later `column:*` moniker when a task lookup misses;
///   that would silently misplace the new entity into an unrelated column
///   scope. `None` bubbles up to the caller, which falls back to the
///   default placement path (lowest-order column).
///
/// Monikers of any other type (`window:`, `field:`, `view:`, …) are
/// skipped — they carry no column information. An empty chain, or a chain
/// with no column/task monikers at all, also returns `None`.
///
/// # Parameters
/// - `scope_chain` — ordered innermost-first list of `type:id` monikers, as
///   produced by `scopeChainFromScope` on the frontend and delivered to
///   the backend via `dispatch_command`.
/// - `task_to_column` — snapshot of the live task → home-column map. The
///   caller materializes this from `EntityContext::list("task")` and the
///   `position_column` field just before dispatch; the resolver itself is
///   pure and storage-free so it stays trivially testable.
///
/// # Returns
/// - `Some(ColumnId)` when the innermost column/task moniker can be
///   resolved.
/// - `None` when the chain contains no column/task moniker, or when the
///   innermost task moniker's id is not in `task_to_column` (see note
///   above about the deliberate "commit on first match" semantics).
pub fn resolve_focused_column(
    scope_chain: &[String],
    task_to_column: &HashMap<TaskId, ColumnId>,
) -> Option<ColumnId> {
    for moniker in scope_chain {
        if let Some(col_id) = moniker.strip_prefix("column:") {
            return Some(ColumnId::from_string(col_id));
        }
        if let Some(task_id) = moniker.strip_prefix("task:") {
            // Commit to this task's lookup. If the caller didn't include
            // the task in the map (unknown task, or an ID that happens to
            // carry extra colons past what `task:` wraps), return `None`
            // rather than silently picking up an outer `column:*`.
            return task_to_column.get(&TaskId::from_string(task_id)).cloned();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage that lives alongside the fn. These mirror the
    //! branch coverage in `tests/resolve_focused_column.rs` so a breaking
    //! change shows up as a unit test failure first — closer to the
    //! implementation — without waiting for the integration-test binary
    //! to compile.

    use super::*;

    #[test]
    fn column_prefix_returns_suffix() {
        let scope = vec!["column:todo".to_string()];
        assert_eq!(
            resolve_focused_column(&scope, &HashMap::new()),
            Some(ColumnId::from_string("todo"))
        );
    }

    #[test]
    fn task_prefix_looks_up_in_map() {
        let mut map = HashMap::new();
        map.insert(TaskId::from_string("t1"), ColumnId::from_string("doing"));
        let scope = vec!["task:t1".to_string()];
        assert_eq!(
            resolve_focused_column(&scope, &map),
            Some(ColumnId::from_string("doing"))
        );
    }

    #[test]
    fn empty_returns_none() {
        assert_eq!(resolve_focused_column(&[], &HashMap::new()), None);
    }

    #[test]
    fn task_not_in_map_returns_none_without_falling_through() {
        let scope = vec!["task:missing".to_string(), "column:todo".to_string()];
        assert_eq!(resolve_focused_column(&scope, &HashMap::new()), None);
    }
}
