---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff80
title: 'Board: new/changed tasks don''t appear live — stale perspective filtered_task_ids'
---
## What

A task created or changed by another process (the `kanban` CLI, the in-process MCP server, an external agent) hot-loads into the kanban-app's entity store via the `entity-created` / `entity-field-changed` Tauri events — but does **not** appear on the board.

Root cause (verified end to end — the watcher/cache/bridge backend all work; the task reaches the store):

- The board renders `tasks ∩ UIState.filtered_task_ids`. `useFilteredEntities` (`apps/kanban-app/ui/src/lib/use-filtered-tasks.ts`) intersects the canonical task list with the per-window `filtered_task_ids`.
- `filtered_task_ids` is a **snapshot** computed once by the `perspective.switch` command (`SwitchPerspectiveCmd` → `evaluate_perspective_filter` in `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs`), written via `UIState::switch_perspective`.
- Nothing recomputes it when tasks change. `usePerspectiveEventListeners` (`apps/kanban-app/ui/src/lib/perspective-context.tsx`) only re-fetches on `perspective`-entity events and `board-changed` — it ignores `task` `entity-created` / `entity-field-changed` / `entity-removed`.

So once any `perspective.switch` has fired (`filtered_task_ids` is `Some`), every task created afterward is absent from the set and `useFilteredEntities` filters it straight out. A never-switched window (`filtered_task_ids == None`) is unaffected — `None` means "no filter, show all".

Chosen fix (per decision): **backend recompute** — when the entity-cache bridge processes a `task` change, recompute each affected window's `filtered_task_ids` server-side and push a `ui-state-changed` event. Keeps filter evaluation server-authoritative; no progress-bar flash, no per-event command roundtrip.

## Approach

In `apps/kanban-app/src/watcher.rs` `run_bridge` / `process_cache_event`: after a cache event for a `task` entity (`EntityCreated`, `EntityRemoved`, `EntityFieldChanged` — including fan-out events), recompute the perspective filter for affected windows.

- The bridge already holds `app: tauri::AppHandle` and `ctx: Arc<KanbanContext>`. Reach `UIState` via `app.state::<crate::state::AppState>().ui_state`.
- For each window in `UIState` whose `filtered_task_ids` is **already `Some`** (skip never-switched `None` windows — leave those to the auto-select path; forcing a snapshot would change their behavior): read its `active_perspective_id`, look up that perspective's filter via `ctx.perspective_context()`, and evaluate it.
- Evaluate with the **same** pipeline `perspective.switch` uses: `evaluate_perspective_filter` in `perspective_commands.rs` is currently a private `async fn` — make it `pub` (or extract a shared `pub` helper) so the bridge and the command share one DSL evaluator with no divergence.
- Dedupe: windows sharing an `active_perspective_id` share a filter — evaluate once per distinct perspective id.
- Call `UIState::switch_perspective(window_label, active_perspective_id, new_ids)` (same id, fresh ids — it is idempotent and returns `Some(UIStateChange::PerspectiveSwitch)` only when the id list actually changed). When it returns `Some`, emit it to that window as a `ui-state-changed` Tauri event, mirroring `emit_ui_state_change_if_needed` in `apps/kanban-app/src/commands.rs`.

Scope: trigger on `task` entity changes only. Tag / project / actor changes can also shift `#tag` / `@user` / `$project` filter membership — note that as a follow-up, out of scope here.

`evaluate_perspective_filter` reloads + enriches all tasks per call; the entity watcher already debounces (~50ms) and the bridge processes events serially, so this is acceptable for board-scale data. Incremental evaluation is a possible later optimization — do not build it now.

## Acceptance Criteria
- [x] A task created by an external process appears on the board live, with no manual perspective switch or refresh, when the window is on a filtered perspective.
- [x] A task field change that moves it into or out of the active perspective's filter updates its board membership live.
- [x] A removed task disappears from the board live.
- [x] Windows on different perspectives each get their own correct recompute; a shared perspective is evaluated once.
- [x] A never-switched window (`filtered_task_ids == None`) is left as `None` — the bridge does not populate it.
- [x] No change to `useFilteredEntities` or the frontend perspective listeners.

## Tests
- [x] Backend integration test (`apps/kanban-app/src/watcher.rs` tests, alongside the existing `resolve_event_*` tests): with a board and a `UIState` window switched to a perspective whose filter matches a tag, add an externally-created task carrying that tag, drive the resulting cache event through the bridge, and assert the window's `filtered_task_ids` now contains the new task id and a `PerspectiveSwitch` change was produced.
- [x] Test that a window with `filtered_task_ids == None` is left untouched after a task cache event.
- [x] Test the dedupe: two windows on the same perspective both get the refreshed id list.
- [x] Unit test for the now-`pub` `evaluate_perspective_filter` (or shared helper) if not already covered.
- [x] Run `cargo test -p kanban-app -p swissarmyhammer-kanban` and `cargo clippy -p kanban-app -- -D warnings` — all green.

## Workflow
- Use `/tdd` — write the failing bridge-recompute test first, then implement.

## Review Findings (2026-05-19 17:15)

Reviewed `apps/kanban-app/src/watcher.rs` and `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs` (uncommitted on branch `kanban`). Verified: `cargo test -p kanban-app -p swissarmyhammer-kanban --no-run` compiles clean; the 4 `recompute_perspective_filters_*` bridge tests and 2 `evaluate_perspective_filter_*` unit tests all pass; `cargo clippy -p kanban-app -- -D warnings` is clean.

Correctness checks that passed: `None` (never-switched) window skip is correct; per-perspective dedupe groups by `active_perspective_id` and evaluates once; the `ui-state-changed` payload `{ "kind": "perspective_switch", "state": <UIState> }` matches the `perspective_switch` discriminator and shape from `emit_ui_state_change_if_needed`; `watch_event_touches_task` correctly matches the three task-typed entity events and excludes `AttachmentChanged`; synthetic fan-out `EntityFieldChanged` events are task-typed so they correctly trigger recompute; no lock-ordering inversion — the `BridgeState` mutex is released before the async recompute, and the `UIState` std `RwLock` is never held across an `.await`; a concurrently-deleted perspective is handled (window left untouched); filter-evaluation errors are logged and the stale snapshot left in place rather than forcing an empty filter. No blockers or warnings.

### Nits
- [x] `apps/kanban-app/src/watcher.rs:18,27,818` — The trigger predicate `watch_event_touches_task` and the `touched_task` gate in `process_cache_event` are never exercised by a test. All four bridge tests call `recompute_perspective_filters` directly, so a regression in the predicate (e.g. accidentally matching `tag` events, or dropping the `task` guard) would pass CI. The task's Tests section explicitly asked to "drive the resulting cache event through the bridge". Consider one test that drives a real `EntityEvent` through `process_cache_event` and asserts a task event triggers recompute while a non-task event does not.
  - Resolved: extracted the gate decision (`resolved.iter().any(watch_event_touches_task)`) from `process_cache_event` into a named `cache_event_touches_task(&[WatchEvent]) -> bool` helper, and added the `cache_event_gate_fires_for_task_events_only` test. It drives a real task `EntityEvent` (`AddTask`) and a real non-task `tag` `EntityEvent` (`cache.write`) end to end through `resolve_event`, then asserts the gate verdict on each resolved batch — task event trips the gate, tag event does not. `process_cache_event` itself needs a real `tauri::AppHandle` (the test module deliberately constructs none — see the `view_event_to_watch_event` doc comment), so the gate logic was extracted to be testable against the same real resolved-event path that `process_cache_event` runs.
- [x] `apps/kanban-app/src/watcher.rs:53-74,92-95` — `recompute_and_emit_perspective_filters` binds the returned `UIStateChange` as `_change` and discards it — it emits a full `to_json()` snapshot instead, mirroring `emit_ui_state_change_if_needed`. Since the `UIStateChange` value is never read, `recompute_perspective_filters` could return `Vec<String>` (the changed window labels) rather than `Vec<(String, UIStateChange)>`, dropping a `clone()` of the id list per changed window and removing the unused-binding noise. Minor; the current shape also leaves the door open for a future per-window typed payload, so this is a judgment call.
  - Resolved: `recompute_perspective_filters` now returns `Vec<String>` (the changed window labels). The discarded `UIStateChange` and its per-window id-list `clone()` are gone, and the `_change` unused binding at the call site is removed. The emit behavior is identical — `recompute_and_emit_perspective_filters` still emits the full `{ "kind": "perspective_switch", "state": <UIState> }` snapshot per changed window. The four `recompute_perspective_filters_*` tests were updated to assert on the returned label vec. The `new_ids.clone()` inside the per-perspective loop is kept (and now commented) — it is genuinely required because `switch_perspective` consumes the id list by value and multiple windows on a shared perspective each need a copy.
