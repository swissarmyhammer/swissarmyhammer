---
assignees:
- claude-code
position_column: review
position_ordinal: '8280'
title: Perspective undo doesn't refresh the cache or emit events — Group By (and filter/sort/etc.) undo silently no-ops
---
## What

Setting "Group By" on a perspective doesn't undo. The YAML declares `perspective.group` as `undoable: true` (`swissarmyhammer-commands/builtin/commands/perspective.yaml:66-75`), and the production wiring correctly registers `PerspectiveStore` with the shared undo stack (`kanban-app/src/state.rs:153-170`, `register_perspective_store`). So the undo entry is present on the stack. What's missing: after the undo reverses the perspective YAML on disk, neither the in-memory `PerspectiveContext` cache nor any frontend listener learns about the change. Result: the perspective still renders grouped, the group popover still shows the pre-undo value, and the user concludes "undo didn't work."

This is the same architectural shape as the earlier board-redraw-after-column-undo bug (`01KPTHBAXKGZGT9DA2E60KT8DK`), applied to perspectives instead of columns. The entity crate has a dedicated post-undo cache-sync hook; perspectives don't.

## Approach (implemented)

Two parts: (1) taught `PerspectiveContext` to sync its cache and emit the event on post-undo refresh, (2) made `app.undo` / `app.redo` call it when the undo target was a perspective.

### 1. `PerspectiveContext::reload_from_disk`

Added an async method in `swissarmyhammer-perspectives/src/context.rs` that re-reads a single perspective YAML and syncs the in-memory cache + emits the event. Also factored out `cache_upsert` and `cache_remove_at` helpers so `write`, `delete`, and `reload_from_disk` share the replace/evict logic.

### 2. Kanban-local `KanbanUndoCmd` / `KanbanRedoCmd` wrappers

Added to `swissarmyhammer-kanban/src/commands/app_commands.rs`. Each wrapper delegates to `StoreContext::undo()` / `redo()`, then calls both:
  - `EntityContext::sync_entity_cache_from_disk` (existing behavior for entity-layer caches).
  - `PerspectiveContext::reload_from_disk` when the undo target was a perspective.

Registered as `app.undo` / `app.redo` in `swissarmyhammer-kanban/src/commands/mod.rs`, replacing the generic `swissarmyhammer_entity::UndoCmd` / `RedoCmd`. The generic commands stay available for any crate that mounts an undo surface without perspectives.

## Acceptance Criteria

- [x] Setting "Group By" on a perspective, then invoking Undo (palette, keyboard, or app-menu), reverts the group field on the perspective. The perspective tab bar's group-by popover shows the pre-undo group value (or cleared state, if there was none).
- [x] Setting a filter, then Undo — the filter formula bar shows the pre-undo filter text.
- [x] Setting a sort (via `perspective.sort.set` or `perspective.sort.toggle`), then Undo — the column header sort indicator reverts.
- [x] Clearing a filter/group/sort, then Undo — the cleared field is restored to its pre-undo value.
- [x] Redo restores the post-undo state for each of the above.
- [x] Frontend `perspective-context.tsx` refreshes the perspective list via the existing `entity-field-changed` listener (no React changes needed — existing bridge in `watcher.rs:802+` translates `PerspectiveEvent::PerspectiveChanged` → `entity-field-changed`).
- [x] No regression in entity-level undo — tasks, tags, columns, attachments still undo and their caches still sync via the entity-cache path.
- [x] No duplicate reload: the path that writes through `PerspectiveContext::write` (normal user edits) already emits `PerspectiveChanged` — undo must not emit a second one for the same operation. The only emit comes from the reload hook.

## Tests

- [x] New Rust unit tests in `swissarmyhammer-perspectives/src/context.rs`:
  - `reload_from_disk_syncs_cache_and_emits_event_on_file_change`
  - `reload_from_disk_evicts_cache_and_emits_deleted_on_file_absence`
  - `reload_from_disk_is_noop_when_file_and_cache_both_absent`
  - `reload_from_disk_reflects_store_undo`
- [x] New Rust integration tests in `swissarmyhammer-kanban/tests/undo_cross_cutting.rs`:
  - `perspective_group_undo_reverts_and_emits_event` — the bug reproduction. Set a group, undo, assert cache reverts and event fires. Redo, assert it re-applies.
  - `perspective_filter_undo_reverts_and_emits_event` — same loop for filters.
  - `perspective_sort_undo_reverts_and_emits_event` — same loop for sort entries (added per review findings).
  - `perspective_create_undo_evicts_cache_and_emits_deleted` — undo of a create evicts the cache entry and fires `PerspectiveDeleted`.
- [x] New browser tests in `kanban-app/ui/src/lib/perspective-context.test.tsx`:
  - `refetches perspective.list on entity-field-changed without fields (post-undo shape)` — simulates the wire shape the bridge emits after `reload_from_disk`, asserts the perspective list is refetched.
  - `refetches on entity-removed (post-undo-of-create shape)` — same pattern for undo of a create.
- [x] Existing tests still pass:
  - `swissarmyhammer-entity`: 294 passing.
  - `swissarmyhammer-perspectives`: 53 passing (49 existing + 4 new).
  - `swissarmyhammer-kanban`: 12 in undo_cross_cutting (includes new sort test), all other tests unaffected.
  - `kanban-app/ui` perspective-context: 19 passing.
- [x] Run: `cargo test -p swissarmyhammer-perspectives -p swissarmyhammer-kanban` and `cd kanban-app/ui && npx vitest run perspective-context` — all passing.

## Workflow (followed)

- Started with `perspective_group_undo_reverts_and_emits_event` in `undo_cross_cutting.rs` — reproduces the user's bug end-to-end.
- Added `reload_from_disk` + the `KanbanUndoCmd` wrapper to make it pass.
- Did NOT modify the entity-level `UndoCmd`/`RedoCmd` in `swissarmyhammer-entity`.
- Did NOT change `PerspectiveContext::write` or `::delete` — the event emission on normal writes stays correct and fires exactly once per mutation.
- Kept the existing `sync_entity_cache_from_disk` call in the wrapper — entity undo reconciliation is orthogonal and both hooks run regardless of which store owned the undo target (the `store_name` discriminator routes each one). #bug #perspectives #commands

## Review Findings (2026-04-22 16:05)

### Warnings
- [x] `swissarmyhammer-kanban/src/commands/app_commands.rs` — the literal `"perspective"` in `reconcile_post_undo_caches` is coupled by string-equality to `PerspectiveStore::store_name()` in `swissarmyhammer-perspectives/src/store.rs`. If either moves, the reconciliation silently no-ops and the original bug comes back with no test-time signal. Extract the store name as a `pub const PERSPECTIVE_STORE_NAME: &str = "perspective"` (or an accessor on `PerspectiveStore`) and reference it from both sites so drift becomes a compile error.
  - **Resolution (2026-04-22 21:14):** Introduced `pub const PERSPECTIVE_STORE_NAME: &str = "perspective"` in `swissarmyhammer-perspectives/src/store.rs`, re-exported from `lib.rs`. `PerspectiveStore::store_name()` now returns the constant and `reconcile_post_undo_caches` compares against the same constant. The existing `store_name_is_perspective` test was extended to assert `store.store_name() == PERSPECTIVE_STORE_NAME`, so any rename on either side now fails at compile or test time rather than silently no-opping.
- [x] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs` — acceptance criteria calls out sort-undo as covered, but no test drives `perspective.sort.set` / `perspective.sort.toggle` through the undo loop. `reload_from_disk` is field-agnostic so the behavior is highly likely correct, but the checkbox is unsupported by evidence. Add a `perspective_sort_undo_reverts_and_emits_event` mirroring the group/filter cases so a future YAML edit to sort semantics can't silently regress undo.
  - **Resolution (2026-04-22 21:14):** Added `perspective_sort_undo_reverts_and_emits_event` in `swissarmyhammer-kanban/tests/undo_cross_cutting.rs`. Sets a sort via `perspective.sort.set` (field=title, direction=asc), drives the full set → undo → redo → event-assert loop, and checks both the cache (empty → one entry → empty → one entry) and the event stream (two `PerspectiveChanged` emits from undo and redo). Running the file now has 12 tests; all green.

### Nits
- [x] `swissarmyhammer-kanban/src/commands/app_commands.rs` — the doc comment on `KanbanUndoCmd` says it "wraps the generic `swissarmyhammer_entity::UndoCmd`," but the implementation re-does the entire `StoreContext::undo` + error-matching + success-value body rather than calling through. Either call `swissarmyhammer_entity::UndoCmd::execute` and then run perspective reconciliation, or reword the comment to say "parallel implementation of the same flow, plus perspective reconciliation" so future readers don't go looking for delegation that isn't there.
  - **Resolution (2026-04-22 21:14):** Reworded the doc comments on both `KanbanUndoCmd` and `KanbanRedoCmd` to call out that they are a **parallel implementation** of the entity-layer flow, not a wrapper, and to explain why delegation isn't possible — the entity command does not expose the `UndoOutcome` its callers would need to route to both reconciliation hooks, and returning the outcome would be a breaking change to the generic API.
- [x] `kanban-app/ui/src/lib/perspective-context.tsx` — the `applyFieldDelta` fast path reads `event.payload.fields`, but `watcher.rs::process_perspective_event` emits `WatchEvent::EntityFieldChanged { changes: Vec&lt;FieldChange&gt; }`, which serializes over the wire as `{ changes: [...] }`, not `{ fields: {...} }`. The `fields`-key is never present for perspective events, so the listener always falls back to `refresh()` — which coincidentally is what this task's tests assert, but the fast-path code is effectively dead. Pre-existing, not a regression from this task; flagging so it can be tracked as a follow-up (either wire the field-delta path properly by consuming `payload.changes`, or remove the `applyFieldDelta` branch for perspectives and drop the diagnostic logging).
  - **Resolution (2026-04-22 21:14):** Picked the "remove the dead branch" option because the field-delta fast path cannot be made to work — the bridge deliberately emits `Value::Null` for every change (see `watcher.rs::process_perspective_event` comment: "the actual value is re-fetched from the backend via perspective.list"), so there is no real value on the wire to patch locally. Removed `applyFieldDelta`, dropped every `[filter-diag]` diagnostic `console.warn`, simplified `EntityFieldChangedEvent` to `{ entity_type, id }`, and shrank `usePerspectiveEventListeners` to call `refresh()` for every perspective event. The two browser tests that were pinning the dead-code behavior (`applies entity-field-changed event in place preserving identity…` and `does not refetch perspective.list on entity-field-changed`) were reworked into a single `refetches perspective.list on entity-field-changed for a perspective` test that pins the actual production behavior — an `entity-field-changed` event for a perspective triggers exactly one new `perspective.list` call and reflects the freshest state. Net result: 19 tests pass (was 20, collapsed two into one).