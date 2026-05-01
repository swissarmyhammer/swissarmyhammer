---
assignees:
- claude-code
depends_on:
- 01KQ5FMAAJZVPC0RT4CVXAGQY9
position_column: todo
position_ordinal: 7f8180
project: single-changelog
title: 'jsingle-changelog: stop the entity dual-writer; delete orphaned entity-layer UndoStack module'
---
#single-changelog #refactor #entity #tech-debt

## Goal

After this card, the entity layer has exactly one changelog writer per `.jsonl` file (the store layer's `ChangelogEntry`), one undo stack module (`swissarmyhammer-store::stack::UndoStack`), and no orphan parallel implementations.

Depends on the projecting reader (`01KQ5FMAAJZVPC0RT4CVXAGQY9`) having landed — without it, turning off the entity-format writer would blank out the history pane and computed-field cache for every new edit, because the existing reader returns no entries for store-format records.

## What

### Stop the duplicate writer

Remove the four production call sites of `swissarmyhammer-entity::changelog::append_changelog`:

- `swissarmyhammer-entity/src/context.rs:387` (create / update path)
- `swissarmyhammer-entity/src/context.rs:461` (delete path)
- `swissarmyhammer-entity/src/context.rs:623` (archive path)
- `swissarmyhammer-entity/src/context.rs:699` (unarchive path)

Verify with `grep -nE 'append_changelog\(' swissarmyhammer-entity/src/context.rs` — line numbers are a 2026-04-26 snapshot.

`pub async fn append_changelog` itself stays exported with `#[deprecated(note = "single-changelog: write through StoreHandle instead")]` because tests use it as a fixture (`changelog.rs` test module, `cache.rs:1525, 2107, 2174, 2336`, `context.rs:3437, 3440`). The cleanup card removes both the function and those test fixtures.

### Delete the orphan UndoStack module

`swissarmyhammer-entity/src/undo_stack.rs` is a ~600-line parallel implementation of `swissarmyhammer-store::stack::UndoStack`. It has its own `UndoStack`, `UndoEntry`, `pub fn load`, `save`, `push`, `undo`, `redo`, `trim`, plus a 7-test module — and is **not declared in `swissarmyhammer-entity/src/lib.rs`**. The `pub mod undo_stack;` line is missing, so the file is never compiled into the crate, never reached, never executed. Pure dead code.

Delete the file. No other changes needed — nothing references it.

### Backwards compat for existing on-disk data

- Old `.kanban/{type}s/{id}.jsonl` files contain a mix of entity-format and store-format records. The projecting reader (card `01KQ5FMAAJZVPC0RT4CVXAGQY9`) handles both shapes.
- Mixed files keep working; the band-aid `is_store_changelog_line` is already deleted by the projecting-reader card.
- Per-edit disk writes drop from 2 to 1.

## Acceptance

- [ ] No production code path in `swissarmyhammer-entity` calls `append_changelog`. Verify: `grep -nE 'append_changelog\(' swissarmyhammer-entity/src/*.rs | grep -v 'mod tests\|#\[cfg(test)\]\|#\[test\]\|#\[tokio::test\]'` returns empty.
- [ ] After running the kanban app and editing a task, `wc -l .kanban/tasks/{id}.jsonl` grows by exactly 1 per edit (was: 2). Verify by tailing the file before and after a single command.
- [ ] No new lines containing `"changes":[` appear in `.kanban/{type}s/*.jsonl` after this card lands. Old lines remain.
- [ ] `swissarmyhammer-entity/src/undo_stack.rs` is deleted. `find swissarmyhammer-entity/src -name 'undo_stack.rs'` returns nothing.
- [ ] `append_changelog` carries `#[deprecated]`; build is clean.
- [ ] Frontend continues to receive `entity-created`, `entity-field-changed`, `entity-removed`, `attachment-changed` events with the same JSON shapes — frontend tests in `kanban-app/ui` unchanged and green.
- [ ] No regression in undo/redo of entity create / update / delete / archive / unarchive — all four `ChangeOp` arms in `StoreHandle::undo` keep working.
- [ ] **Perspective regression**: `swissarmyhammer-kanban/tests/undo_cross_cutting.rs::perspective_delete_undo_restores_cache_and_emits_event` (line 1031) passes unchanged. This card touches `swissarmyhammer-store::StoreContext::undo` and `sync_entity_cache_from_disk`, both shared with the perspective undo path. The perspective test is the canonical guard that perspective-create / perspective-deleted Tauri events still flow correctly.
- [ ] No regression in perspectives' on-disk shape: `.kanban/perspectives/*.jsonl` continues to contain only store-format records, written by `StoreHandle<PerspectiveStore>` (single writer).
- [ ] `cargo nextest run -p swissarmyhammer-entity -p swissarmyhammer-kanban -p kanban-app -p swissarmyhammer-perspectives -p swissarmyhammer-views` green.
- [ ] `cargo nextest run -p swissarmyhammer-kanban --test undo_cross_cutting` green (the cross-cutting test suite covers entity + perspective delete-undo together).

## Tests — focus on delete/undo-delete event roundtrips

The most failure-prone path is "user deletes X, then undoes the delete, the UI must restore X." Lock this down at every layer for entities, and assert the perspective regression as a guard.

### Entity layer (write these)

- [ ] `swissarmyhammer-entity/src/context.rs` — `write_does_not_append_to_entity_changelog`: `EntityContext::write` against tempdir, assert per-entity `.jsonl` has zero `"changes":[` lines.
- [ ] `swissarmyhammer-entity/src/context.rs` — `delete_does_not_append_to_entity_changelog`: `EntityContext::delete`, same assertion.
- [ ] `swissarmyhammer-entity/src/context.rs` — `delete_then_undo_round_trip_emits_correct_events`: subscribe to `EntityCache::subscribe()`, write a task (`EntityChanged` fires), delete it (`EntityDeleted` fires), `StoreContext::undo()` to restore (`EntityChanged` fires via `sync_entity_cache_from_disk`), redo (`EntityDeleted` fires again). Assert the event sequence and that the entity is `read()`-able after undo and not after redo.
- [ ] `swissarmyhammer-entity/src/context.rs` — `archive_then_undo_round_trip_emits_correct_events`: same shape for archive/unarchive paths.
- [ ] `kanban-app/src/watcher.rs` — `bridge_routes_undo_of_delete_to_entity_created`: end-to-end through the bridge. Real `EntityCache`, real bridge, fake Tauri emitter. Write → delete → undo. Assert the emitter receives, in order: `entity-created`, `entity-removed`, `entity-created`. The third event is `entity-created` (not `entity-field-changed`) because the bridge's seen-set was cleared on delete and the post-undo `EntityChanged` finds the key absent.

### Perspective layer (regression guards — already exist)

- [ ] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs::perspective_delete_undo_restores_cache_and_emits_event` (line 1031) passes unchanged. This is the test that proves perspective-create / perspective-deleted events still flow through the shared `StoreContext::undo` infrastructure. It walks: create perspective → assert cache + file present → delete → assert `PerspectiveDeleted` event + file gone + cache empty → undo → assert file restored + cache restored + `PerspectiveChanged { is_create: false }` event → redo → assert removal. The bridge maps these to `entity-created` / `entity-removed` / `entity-field-changed` Tauri events with `entity_type: "perspective"`.

### Cross-cutting

- [ ] `cargo nextest run -p swissarmyhammer-entity` green.
- [ ] `cargo nextest run -p kanban-app` green.
- [ ] `cargo nextest run -p swissarmyhammer-kanban --test undo_cross_cutting` green.

## Workflow

`/tdd` — start with the round-trip tests in `EntityContext` (they characterize the contract). Then the bridge round-trip test. Then the two no-disk-write tests. Then delete the four `append_changelog` call sites and `undo_stack.rs`. Run the cross-cutting tests; the perspective regression should pass without modification — if it doesn't, the writer-off changes broke shared infrastructure.

## Scope

- depends_on: `01KQ5FMAAJZVPC0RT4CVXAGQY9` (projecting reader).
- Blocks: nothing strictly. Independent of the views card and the cleanup card.
