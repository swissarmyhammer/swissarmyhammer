---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 7f80
project: single-changelog
title: 'single-changelog: migrate views to StoreHandle&lt;ViewStore&gt;; delete ViewsChangelog and hand-rolled undo'
---
#single-changelog #refactor #views #tech-debt

## Goal

Views become the third domain on the same unified machinery already used by entities and perspectives: text-patch `ChangelogEntry` written by `StoreHandle<ViewStore>`, undo through the global `undo_stack.yaml`, and a Tauri event bridge so the UI learns about view create / change / delete / undo-of-delete in real time. Drop the parallel `ViewsChangelog` + `ViewChangeEntry` (whole-document JSON snapshots) + hand-rolled `undo_entry`.

Independent of the other `single-changelog` cards.

## Why this is the right shape

Today views diverge from the unified design in three ways:

- **Storage shape**: `ViewChangeEntry` carries the full `previous` and `current` ViewDef as JSON on every entry (`swissarmyhammer-views/src/changelog.rs:33-37`). Strictly worse than the diffy text patches in `ChangelogEntry`.
- **Undo machinery**: `ViewsChangelog::undo_entry` (`changelog.rs:95-154`) is hand-rolled and doesn't push onto `undo_stack.yaml`. Cmd-Z on a view edit doesn't go on the global undo stack today.
- **Events**: `swissarmyhammer-views` has no `broadcast`, `subscribe`, or event channel of any kind (verified by grep). The kanban-app has no path for `view-created` / `view-removed` Tauri events. Today, the UI must already have view state and update it client-side; there's no server-side event that announces a view delete or undo-of-delete.

Migrating to `StoreHandle<ViewStore>` fixes all three.

## Bridge contract: mirror the perspective pattern, not the entity pattern

There are two pre-existing patterns in the codebase for routing domain events to the UI:

- **Entity** uses a per-board `HashSet<(entity_type, id)>` seen-set in the bridge. The bridge decides `entity-created` vs `entity-field-changed` based on membership. Undo of delete emits `EntityChanged` (because the cache reload sees a "new" entity), the bridge sees the key absent from the seen-set, and emits `entity-created`.
- **Perspective** uses an `is_create: bool` flag carried on the `PerspectiveChanged` event itself (`swissarmyhammer-perspectives/src/events.rs`). `reload_from_disk` always emits `is_create: false`, so undo of delete emits `entity-field-changed` (with `entity_type: "perspective"`). The frontend perspective listener treats any change event for a perspective as "refetch the list."

**This card adopts the perspective pattern for views.**

- Add `swissarmyhammer-views/src/events.rs` mirroring `swissarmyhammer-perspectives/src/events.rs`: `enum ViewEvent { ViewChanged { id, changed_fields, is_create }, ViewDeleted { id } }`.
- `ViewsContext` carries `event_sender: broadcast::Sender<ViewEvent>` and emits on `write_view`/`delete_view`/`reload_from_disk`. `reload_from_disk` always emits `is_create: false` — same convention as perspectives.
- The kanban-app bridge (`kanban-app/src/watcher.rs`) gets a `process_view_event` mirroring `process_perspective_event` (`watcher.rs:802`), routing into the same `entity-created` / `entity-field-changed` / `entity-removed` Tauri channels with `entity_type: "view"`. Frontend treats any `entity_type === "view"` event as a refetch trigger.

Why this pattern: views are configuration, the list is small (typically &lt;10 per board), refetch-on-any-change is cheap, and the existing perspective bridge code is straightforward to mirror. The entity seen-set pattern is overkill for this volume.

## What

### Add `impl TrackedStore for ViewStore`

Mirror `swissarmyhammer-perspectives/src/store.rs`:

- New `swissarmyhammer-views/src/store.rs`. `ViewStore { root: PathBuf, ... }` with `serialize` / `deserialize` for `ViewDef` ↔ YAML. `extension() = "yaml"`.
- `impl swissarmyhammer_store::store::sealed::Sealed for ViewStore {}`.
- `impl TrackedStore for ViewStore { type Item = ViewDef; type ItemId = ViewId; ... }`.

### Add `swissarmyhammer-views::events::ViewEvent`

Mirror `swissarmyhammer-perspectives::events::PerspectiveEvent`:

- `enum ViewEvent { ViewChanged { id: String, changed_fields: Vec<String>, is_create: bool }, ViewDeleted { id: String } }`.

### Wire `StoreHandle<ViewStore>` + event channel into `ViewsContext`

Mirror `PerspectiveContext::set_store_handle` (`perspectives/context.rs:235-243`) and event-channel mechanics (`perspectives/context.rs:52, 126, 228, 293, 304`):

- Add `store_handle: Option<Arc<StoreHandle<ViewStore>>>`, `store_context: Option<Arc<StoreContext>>`, `event_sender: broadcast::Sender<ViewEvent>` fields.
- `pub fn subscribe(&self) -> broadcast::Receiver<ViewEvent>`.
- `write_view` / `delete_view` delegate to `StoreHandle` and emit `ViewChanged` / `ViewDeleted`.
- `reload_from_disk(view_id)` (called from `UndoCmd`/`RedoCmd`) re-reads the file and emits `ViewChanged { is_create: false }` if present, `ViewDeleted` if gone.

### Register in `kanban-app::state` + bridge

- New `register_view_store(...)` mirroring `register_perspective_store` (`kanban-app/src/state.rs:154-171`).
- New `process_view_event` in `kanban-app/src/watcher.rs` mirroring `process_perspective_event` (`watcher.rs:802`), routing to `entity-*` Tauri events with `entity_type: "view"`.
- Spawn a view-event subscriber alongside the entity-cache subscriber in the bridge boot.

### Delete the old machinery

- Delete `swissarmyhammer-views/src/changelog.rs` entirely (`ViewChangeEntry`, `ViewChangeOp`, `ViewsChangelog`, `undo_entry`, `append_changelog`, `read_changelog`, `log_create`/`log_update`/`log_delete`).
- Delete `views_changelog: ViewsChangelog` from `KanbanContext` (`swissarmyhammer-kanban/src/context.rs:120`). Replace with `StoreHandle<ViewStore>` wiring.
- Delete every Tauri command and entry point that called `ViewsChangelog::log_*` or `undo_entry` directly. They route through the unified `UndoCmd` / `RedoCmd`.
- Delete `swissarmyhammer-views/src/error.rs::ViewsError::ChangelogEntryNotFound` and `NothingToUndo`.

### Old `views.jsonl` data

Leave on disk. The cleanup card (`01KQ5QPDWXT1VGV8RE9NKR2F1A`) deletes it.

## Acceptance

- [ ] `swissarmyhammer-views/src/store.rs` exists with `impl TrackedStore for ViewStore`.
- [ ] `swissarmyhammer-views/src/events.rs` exists with `ViewEvent::{ViewChanged, ViewDeleted}` mirroring `PerspectiveEvent`.
- [ ] `swissarmyhammer-views/src/changelog.rs` is **deleted**.
- [ ] `grep -rn 'ViewsChangelog\|ViewChangeEntry\|ViewChangeOp\|views_changelog' --include='*.rs'` returns nothing outside legacy migration tests.
- [ ] `ViewsContext::write_view` / `delete_view` push onto the global `UndoStack` (`undo_stack.yaml`).
- [ ] `UndoCmd` works on a view edit: write a view, run `UndoCmd`, the previous view state is restored on disk and `ViewsContext` reflects it.
- [ ] Frontend receives Tauri events `entity-created` / `entity-field-changed` / `entity-removed` with `entity_type: "view"` for view operations.
- [ ] **Perspective regression**: `cargo nextest run -p swissarmyhammer-kanban --test undo_cross_cutting perspective_delete_undo_restores_cache_and_emits_event` passes unchanged. (This card touches shared bridge infrastructure that perspectives also depend on.)
- [ ] `cargo nextest run -p swissarmyhammer-views -p swissarmyhammer-kanban -p kanban-app -p swissarmyhammer-perspectives` green.

## Tests — focus on delete/undo-delete event roundtrips for ALL three domains

The bridge changes touch shared code paths. Lock down all three.

### Views (new behavior — write these)

- [ ] `swissarmyhammer-views/src/context.rs` — `write_view_pushes_onto_undo_stack`: regression equivalent to `swissarmyhammer-perspectives::context::write_pushes_onto_undo_stack` (`context.rs:1101`).
- [ ] `swissarmyhammer-views/src/context.rs` — `delete_view_pushes_onto_undo_stack`: equivalent to perspectives' `delete_pushes_onto_undo_stack` (`context.rs:1131`).
- [ ] `swissarmyhammer-views/src/context.rs` — `undo_view_create_round_trip`: write → assert file + undo entry → `StoreContext::undo()` → assert file gone, cache empty, `ViewDeleted` event fires.
- [ ] `swissarmyhammer-views/src/context.rs` — `undo_view_delete_round_trip`: write → delete → assert file in `.trash/`, cache empty, `ViewDeleted` event → `StoreContext::undo()` → assert file restored, cache restored, `ViewChanged { is_create: false }` event. Mirrors `swissarmyhammer-kanban/tests/undo_cross_cutting.rs:1031 perspective_delete_undo_restores_cache_and_emits_event`.
- [ ] `kanban-app/src/watcher.rs` — `bridge_routes_view_undo_of_delete`: end-to-end. Real ViewsContext, real StoreHandle, fake Tauri emitter. Write view → delete → undo. Assert the emitter receives, in order: `entity-created` (entity_type=view, is_create=true), `entity-removed` (entity_type=view), `entity-field-changed` (entity_type=view, on undo — perspective pattern). The third event is `entity-field-changed` because `reload_from_disk` emits `is_create: false` per the bridge contract above.
- [ ] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs` — `view_delete_undo_restores_cache_and_emits_event`: integration test mirroring `perspective_delete_undo_restores_cache_and_emits_event` for views.

### Entities (regression guards)

- [ ] No regression in `kanban-app/src/watcher.rs` `bridge_end_to_end_*` tests — they continue to assert `entity-created` on undo of delete via the seen-set pattern.

### Perspectives (regression guards)

- [ ] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs::perspective_delete_undo_restores_cache_and_emits_event` (line 1031) passes unchanged. This is the cross-cutting test that proves perspective create/delete/undo events still flow through the shared bridge infrastructure after the views work lands.

## Workflow

`/tdd`. Start with the four `swissarmyhammer-views::context` roundtrip tests against the mirror of `PerspectiveStore` — they fail because `ViewStore` doesn't exist yet. Build `ViewStore`, `ViewEvent`, `set_store_handle`, `reload_from_disk` until they pass. Add the bridge test next; build `process_view_event`. Then port the integration test from perspectives. Then delete the old `changelog.rs`. Run the full nextest including the perspective regression guard.

## Scope

- depends_on: nothing.
- Blocks: cleanup card (`01KQ5QPDWXT1VGV8RE9NKR2F1A`) which removes `.kanban/views.jsonl`.
