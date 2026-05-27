---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff080
title: 'Board empty on launch: to_json emits filtered_task_ids: [] for never-switched windows'
---
## What

On launch, the kanban board shows **no tasks** under the active (unfiltered) perspective until the user clicks away to another perspective and back.

**Root cause:** `UIState::to_json()` in `crates/swissarmyhammer-commands/src/ui_state.rs` unconditionally injects `filtered_task_ids` into every window's snapshot:

```rust
map.insert(
    "filtered_task_ids".to_string(),
    serde_json::json!(ws.filtered_task_ids),
);
```

On a fresh boot `ws.filtered_task_ids` is `Vec::new()`, so the wire sends `filtered_task_ids: []`.

The frontend filter selector `apps/kanban-app/ui/src/lib/use-filtered-tasks.ts` is a deliberate **tri-state**:
- `undefined` — no `perspective.switch` fired yet → show all tasks
- `[]` — switch fired, filter matched zero → empty board
- non-empty — intersect

Because `to_json` always emits `[]`, the "never switched" state is indistinguishable from "switched, matched zero". `useFilteredEntities` honors `[]` and renders an empty board. The repair hook `useAutoSelectActivePerspective` in `apps/kanban-app/ui/src/lib/perspective-context.tsx` (repair path 2) only redispatches `perspective.switch` when `!filtered_task_ids_defined` — but `[] !== undefined`, so it never fires. Switching perspectives manually calls `perspective.switch`, which recomputes the real id list and populates the board.

The backend filter evaluation itself is correct (`evaluate_perspective_filter` + `filter_task_ids` return every id for an empty filter). The defect is purely that the initial snapshot cannot represent the third ("never switched") state on the wire.

## Approach

Make the "never switched" state representable on the wire by changing the backend field to an `Option`:

- [x] In `crates/swissarmyhammer-commands/src/ui_state.rs`, change `WindowState.filtered_task_ids` from `Vec<String>` to `Option<Vec<String>>`. `None` = never switched; `Some(vec)` = a `perspective.switch` has occurred.
- [x] Update the `Default`/constructor for `WindowState` to initialize `filtered_task_ids: None`.
- [x] `UIState::switch_perspective(window_label, perspective_id, filtered_task_ids: Vec<String>)` — keep the public signature; set `ws.filtered_task_ids = Some(...)`. Update the `ids_changed` comparison accordingly.
- [x] `UIState::filtered_task_ids(window_label) -> Vec<String>` — keep the public signature; resolve via `.flatten().unwrap_or_default()`.
- [x] In `to_json`, inject the `filtered_task_ids` key only when the window's value is `Some`; omit it entirely when `None` so the frontend receives `undefined`.
- [x] The `UIStateChange::PerspectiveSwitch { filtered_task_ids: Vec<String> }` variant stays unchanged — switch events always carry a concrete list.

The frontend needs no change: `WindowStateSnapshot.filtered_task_ids` is already `?: string[]`, and `useFilteredEntities` + `useAutoSelectActivePerspective` already treat `undefined` as "no switch yet". The field `filtered_task_ids` is only directly accessed inside `ui_state.rs` (other crates go through the `switch_perspective` / `filtered_task_ids()` API), so the blast radius is contained to that file.

## Acceptance Criteria

- [x] A fresh `UIState` (no `switch_perspective` ever called), serialized via `to_json()`, produces a window object with **no** `filtered_task_ids` key (frontend reads it as `undefined`).
- [x] After `switch_perspective`, `to_json()` includes `filtered_task_ids` as the concrete array.
- [x] `UIState::filtered_task_ids(window)` still returns an empty `Vec` for a never-switched window (public behavior unchanged).
- [x] `switch_perspective` no-op detection still works (same id + same ids → `None`).
- [x] `cargo build -p swissarmyhammer-commands` and the existing kanban crates compile with no API changes outside `ui_state.rs`.

## Tests

- [x] `/tdd` regression test in the `#[cfg(test)]` module of `crates/swissarmyhammer-commands/src/ui_state.rs`, next to `switch_perspective_appears_in_to_json`: a fresh `UIState` with a window present (e.g. via any window-touching call) serialized through `to_json()` must NOT contain a `filtered_task_ids` key — fails before the fix (currently emits `[]`), passes after.
- [x] Test: after `switch_perspective("main", "p1", vec!["t1"])`, `to_json()` includes `filtered_task_ids == ["t1"]` (keep/adapt `switch_perspective_appears_in_to_json`).
- [x] Test: `filtered_task_ids("main")` returns an empty `Vec` for a never-switched window.
- [x] Confirm existing tests still pass: `switch_perspective_noop_returns_none`, `switch_perspective_emits_change_when_only_ids_change`, `filtered_task_ids_not_persisted`, `switch_perspective_sets_both_fields_atomically`.
- [x] Run: `cargo test -p swissarmyhammer-commands ui_state` — all green.

## Workflow

- Use `/tdd` — write the failing `to_json` regression test first, watch it fail (current `to_json` emits `[]`), then implement the `Option` change to make it pass.