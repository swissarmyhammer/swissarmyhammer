---
assignees:
- claude-code
depends_on:
- 01KPVE8GGBX6Q5SEDGGTKKS4CB
position_column: todo
position_ordinal: ff8380
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

- [ ] Right-click a perspective tab, select "Delete Perspective". Tab disappears from the tab bar.
- [ ] Invoke Undo (palette, keyboard, or app-menu). The deleted tab reappears in the tab bar with its original name and fields.
- [ ] After the undo, clicking the restored tab activates it correctly (name, filter, group, sort all intact — no stale state).
- [ ] Redo removes it again; a subsequent undo restores it again — the cycle works both ways.
- [ ] The Rust-level `PerspectiveContext::get_by_id(id)` returns `Some` after undo (cache, not just disk).
- [ ] A `PerspectiveEvent::PerspectiveChanged` event fires on the broadcast channel post-undo (so Tauri bridge + frontend can react).
- [ ] No regression in existing delete flow — `ctx.delete(id)` still emits `PerspectiveEvent::PerspectiveDeleted` and the tab disappears as before.

## Tests

- [ ] New Rust integration test `perspective_delete_undo_restores_cache_and_emits_event` in `swissarmyhammer-kanban/tests/undo_cross_cutting.rs`:
  1. Set up KanbanContext + StoreContext + register perspective store (same pattern as the parent task's test).
  2. Create a perspective. Subscribe to `PerspectiveEvent` broadcast.
  3. Dispatch `perspective.delete` for that perspective. Assert it's gone from `pctx.all()` and the tab's YAML file is absent on disk.
  4. Dispatch `app.undo`.
  5. Assert the YAML file is back on disk (existing store-level guarantee).
  6. Assert `pctx.get_by_id(id).is_some()` — the **cache** has the perspective restored.
  7. Assert at least one `PerspectiveEvent::PerspectiveChanged` (with matching id) was received on the broadcast post-undo.
- [ ] New Rust unit test `reload_from_disk_reinserts_previously_deleted_perspective` in `swissarmyhammer-perspectives/src/context.rs` tests module (adjacent to the parent task's `reload_from_disk_*` tests):
  1. Open context. Create perspective X. Delete it (cache + disk gone).
  2. Manually write the YAML for X back to disk (simulating post-undo store rewrite).
  3. Call `pctx.reload_from_disk("X_id")`.
  4. Assert `pctx.get_by_id("X_id").is_some()` and the broadcast received `PerspectiveChanged` with `is_create: false`.
- [ ] New browser test `kanban-app/ui/src/components/perspective-tab-bar.delete-undo.test.tsx`:
  1. Mount `PerspectiveTabBar` with a `PerspectiveProvider` wired to a mock Tauri bridge.
  2. Mock `perspective.list` to return 2 perspectives, seed and render.
  3. Dispatch `perspective.delete` via the mock; assert the tab count drops to 1.
  4. Fire a synthetic `entity-field-changed` Tauri event for the deleted perspective's id with empty `changes` (simulating what the bridge emits post-undo).
  5. Assert `perspective.list` is refetched and the restored tab renders again.
- [ ] Existing tests still pass:
  - `delete_pushes_onto_undo_stack` (`swissarmyhammer-perspectives/src/context.rs:1066-1087`) — file-level undo unchanged.
  - Parent task's tests (`reload_from_disk_syncs_cache_and_emits_event_on_file_change`, `perspective_group_undo_reverts_and_emits_event`) — cache-sync mechanics unchanged.
- [ ] Run: `cargo nextest run -p swissarmyhammer-perspectives -p swissarmyhammer-kanban` and `cd kanban-app/ui && bun test perspective-tab-bar.delete-undo` — all passing.

## Workflow

- Use `/tdd`. Write `perspective_delete_undo_restores_cache_and_emits_event` first; it should fail against today's code (the cache isn't refreshed) and pass after `01KPVE8GGBX6Q5SEDGGTKKS4CB` lands.
- If you hit the "tests pass but UI still broken" branch from the Approach section, the follow-up fixes belong in `watcher.rs` or `perspective-context.tsx`, not in the perspectives crate.
- Do NOT re-implement `reload_from_disk` or the undo wrapper — those come from the parent task. This task is coverage + UI verification.
- Do NOT touch `PerspectiveContext::delete` — its current emission of `PerspectiveDeleted` on the forward path is correct. #bug #perspectives #commands