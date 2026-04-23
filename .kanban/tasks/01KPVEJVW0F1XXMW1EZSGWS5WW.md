---
assignees:
- claude-code
depends_on:
- 01KPVE8GGBX6Q5SEDGGTKKS4CB
position_column: done
position_ordinal: ffffffffffffffffffffffff9280
title: 'Perspective delete undo: verify frontend restoration, add end-to-end test coverage'
---
## What

Deleting a perspective (via `perspective.delete` / context menu "Delete Perspective") then invoking Undo does not restore the perspective tab. The file on disk DOES get restored by the store-level undo (verified — see existing test `delete_pushes_onto_undo_stack` at `swissarmyhammer-perspectives/src/context.rs:1066-1087`), but the in-memory `PerspectiveContext` cache still has the perspective removed (via `swap_remove` at line 229) and no `PerspectiveEvent::PerspectiveChanged` fires after undo — so the frontend tab bar never learns the perspective came back.

**This has the same root cause as `01KPVE8GGBX6Q5SEDGGTKKS4CB` (Group By undo).** The proposed `PerspectiveContext::reload_from_disk` method in that task already handles the file-reappears case:

```rust
if path.exists() {
    self.replace_or_insert(p.clone());  // re-inserts even if evicted
    emit PerspectiveChanged { is_create: false, ... };
} else {
    self.remove_by_id(id);
    emit PerspectiveDeleted;
}
```

Both branches work for the delete-undo scenario:
- File was deleted → cache removed it → store-level undo restores file → `reload_from_disk` sees file exists → `replace_or_insert` adds it back → emits PerspectiveChanged → bridge at `kanban-app/src/watcher.rs:802+` translates to `entity-field-changed` (with empty `changed_fields`) → frontend `perspective-context.tsx` listener refetches the list → tab reappears.

So the fix delivered by `01KPVE8GGBX6Q5SEDGGTKKS4CB` should cover this symptom automatically. **What's missing is explicit test coverage and an acceptance criterion for the delete-undo path** — the parent task's tests focus on field-level changes (group/filter/sort) and file-absent eviction, not the file-reappears-after-delete case.

This task adds that coverage and verifies the UI actually shows the restored perspective.

## Approach

**Do NOT duplicate the `reload_from_disk` implementation or the `KanbanUndoCmd` wrapper — those come from `01KPVE8GGBX6Q5SEDGGTKKS4CB`.** This task is pure test + verification.

### If `01KPVE8GGBX6Q5SEDGGTKKS4CB` has already landed

1. Add the delete-undo test cases below (Tests section).
2. Run through the manual acceptance-criteria list and confirm the tab reappears.

### If the fix is not yet merged

1. Block on `01KPVE8GGBX6Q5SEDGGTKKS4CB` (declared as `depends_on`).
2. When that lands, run this task's tests. If any fail because of a gap in `reload_from_disk`'s file-reappears handling, fold the gap fix back into the parent task; this task stays test-only.

### If the tests pass but the UI still doesn't restore the tab

The bridge + frontend listener chain is already verified for field changes (parent task's acceptance criteria). A failure here would indicate either:
- The bridge translates empty `changed_fields` into a no-op instead of a refetch. Fix in `kanban-app/src/watcher.rs:802-830` to treat `changed_fields: []` on a perspective as "refetch signal."
- The frontend listener in `kanban-app/ui/src/lib/perspective-context.tsx:167-189` skips events for entities not already in its local state. Fix in that listener to refetch on any perspective event regardless of local presence.

Both are small, targeted follow-ups that only surface after running the new test cases.

## Acceptance Criteria

- [x] Right-click a perspective tab, select "Delete Perspective". Tab disappears from the tab bar. (Forward path unchanged from parent task; verified by `perspective_delete_undo_restores_cache_and_emits_event` asserting cache/disk/event after delete.)
- [x] Invoke Undo (palette, keyboard, or app-menu). The deleted tab reappears in the tab bar with its original name and fields. (Verified by the Rust integration test + `perspective-tab-bar.delete-undo.test.tsx` browser test — tab count goes 2 → 1 → 2 and the DOM shows the restored tab name.)
- [x] After the undo, clicking the restored tab activates it correctly (name, filter, group, sort all intact — no stale state). (Unit test `reload_from_disk_reinserts_previously_deleted_perspective` asserts `name`, `group`, and `filter` are all restored in the cache after reload — the selector path reads those fields unchanged.)
- [x] Redo removes it again; a subsequent undo restores it again — the cycle works both ways. (`perspective_delete_undo_restores_cache_and_emits_event` drives delete → undo → redo → undo and asserts cache/disk/event state at every step.)
- [x] The Rust-level `PerspectiveContext::get_by_id(id)` returns `Some` after undo (cache, not just disk). (Asserted directly in both Rust tests.)
- [x] A `PerspectiveEvent::PerspectiveChanged` event fires on the broadcast channel post-undo (so Tauri bridge + frontend can react). (Asserted in both Rust tests; the event has `is_create: false` and empty `changed_fields`, matching the bridge's refetch-signal contract.)
- [x] No regression in existing delete flow — `ctx.delete(id)` still emits `PerspectiveEvent::PerspectiveDeleted` and the tab disappears as before. (Full perspectives + kanban suite green — 1303 tests pass, including the original `delete_pushes_onto_undo_stack` and `test_delete_perspective_emits_item_removed_event`.)

## Tests

- [x] New Rust integration test `perspective_delete_undo_restores_cache_and_emits_event` in `swissarmyhammer-kanban/tests/undo_cross_cutting.rs`:
  1. Set up KanbanContext + StoreContext + register perspective store (same pattern as the parent task's test).
  2. Create a perspective. Subscribe to `PerspectiveEvent` broadcast.
  3. Dispatch `perspective.delete` for that perspective. Assert it's gone from `pctx.all()` and the tab's YAML file is absent on disk.
  4. Dispatch `app.undo`.
  5. Assert the YAML file is back on disk (existing store-level guarantee).
  6. Assert `pctx.get_by_id(id).is_some()` — the **cache** has the perspective restored.
  7. Assert at least one `PerspectiveEvent::PerspectiveChanged` (with matching id) was received on the broadcast post-undo.
  - Also drives `app.redo` then a second `app.undo` to pin the full cycle.
- [x] New Rust unit test `reload_from_disk_reinserts_previously_deleted_perspective` in `swissarmyhammer-perspectives/src/context.rs` tests module (adjacent to the parent task's `reload_from_disk_*` tests):
  1. Open context. Create perspective X (with `group` and `filter` set to non-default values for round-trip assertions). Delete it (cache + disk gone).
  2. Manually write the YAML for X back to disk (simulating post-undo store rewrite).
  3. Call `pctx.reload_from_disk("X_id")`.
  4. Assert `pctx.get_by_id("X_id").is_some()`, the restored perspective carries the original name/group/filter, and the broadcast received `PerspectiveChanged` with `is_create: false` and empty `changed_fields`.
- [x] New browser test `kanban-app/ui/src/components/perspective-tab-bar.delete-undo.test.tsx`:
  1. Mount `PerspectiveTabBar` inside the **real** `PerspectiveProvider` (ancillary contexts — schema, UIState, views, entity store, board data, context menu — are stubbed at the `vi.mock` boundary; only `@tauri-apps/api/{core,event}` is mocked for the bridge).
  2. Mock `perspective.list` to return 2 board-kind perspectives, render, and assert both tabs show.
  3. Fire synthetic `entity-removed` Tauri event for the deleted perspective; prime next `perspective.list` with the post-delete list; assert the tab count drops to 1.
  4. Fire a synthetic `entity-field-changed` Tauri event for the deleted perspective's id with empty `changes` (the exact wire shape the bridge emits post-undo). Prime next `perspective.list` with the restored 2-perspective list.
  5. Assert `perspective.list` is re-invoked exactly once more (refetch counter), and the restored tab renders again in the DOM.
- [x] Existing tests still pass:
  - `delete_pushes_onto_undo_stack` (`swissarmyhammer-perspectives/src/context.rs:1131`) — file-level undo unchanged.
  - Parent task's tests (`reload_from_disk_syncs_cache_and_emits_event_on_file_change`, `perspective_group_undo_reverts_and_emits_event`, etc.) — cache-sync mechanics unchanged.
  - All 19 existing `perspective-context.test.tsx` tests pass. All 28 existing `perspective-tab-bar.test.tsx` tests pass.
- [x] Ran: `cargo nextest run -p swissarmyhammer-perspectives -p swissarmyhammer-kanban` → **1303/1303 passing**. `cd kanban-app/ui && npx vitest run src/lib/perspective-context.test.tsx src/components/perspective-tab-bar.test.tsx src/components/perspective-tab-bar.delete-undo.test.tsx` → **48/48 passing**.

## Workflow

- Used `/tdd`. Wrote `perspective_delete_undo_restores_cache_and_emits_event` first; it passed on top of the parent-task infrastructure without any additional Rust changes — confirming the delete-undo symptom was already covered by the `reload_from_disk` file-reappears branch that `KanbanUndoCmd` invokes.
- Did not hit the "tests pass but UI still broken" branch — the existing bridge + listener chain handles the empty-`changes` refetch signal correctly.
- Did NOT re-implement `reload_from_disk` or the undo wrapper — those come from the parent task. This task is coverage + UI verification.
- Did NOT touch `PerspectiveContext::delete` — its current emission of `PerspectiveDeleted` on the forward path is correct. #bug #perspectives #commands

## Implementation Notes

- The unit test subscribes to the broadcast AFTER the initial delete so the post-reload event is the first thing the receiver sees — no drained noise to filter out.
- The integration test subscribes AFTER the initial create but drains the delete event explicitly before asserting the undo event, matching the parent task's pattern in `perspective_group_undo_reverts_and_emits_event`.
- The browser test mounts `PerspectiveTabBar` inside the **real** `PerspectiveProvider` (not a mocked `usePerspectives`) — this is the crucial difference from the existing `perspective-tab-bar.test.tsx`, which mocks the context entirely. The goal is to verify the real event-listener → refetch → re-render loop.