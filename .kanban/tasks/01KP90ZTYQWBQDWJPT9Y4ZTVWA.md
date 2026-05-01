---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd080
title: Perspective mutations must emit events so filter changes apply live
---
## What

The filter bar saves changes (perspective YAML is written), but the view does not re-apply the filter until the user switches perspectives and back. The filter must feel live — update the visible task list as the user types.

### Root Cause

Perspectives write through `PerspectiveContext` / `PerspectiveStore`, which is a parallel system to `EntityCache`. The bridge in `kanban-app/src/watcher.rs` subscribes **only** to `EntityCache` and translates events into Tauri `entity-field-changed` / `entity-created` / `entity-removed` events. Since `PerspectiveContext::write()` never emits anything on a broadcast channel, **no Tauri event reaches the frontend** when a perspective field (like `filter`) changes.

The frontend `PerspectiveProvider` (`kanban-app/ui/src/lib/perspective-context.tsx:108-124`) listens for `entity-field-changed` with `entity_type === "perspective"` and calls `refresh()` — but this event never arrives. So `activePerspective` stays stale, `PerspectiveContainer`'s `useEffect([activeFilter])` (`perspective-container.tsx:102-105`) never fires, and `refreshEntities(boardPath, activeFilter)` is never called with the new filter.

### Fix

Add a `tokio::sync::broadcast` channel to `PerspectiveContext` (following the `EntityCache` pattern). When `write()` or `delete()` is called, broadcast a `PerspectiveEvent`. The bridge in `watcher.rs` subscribes to this channel alongside the `EntityCache` channel and translates perspective events into the same Tauri event shape (`entity-field-changed` with `entity_type: "perspective"`). The frontend already listens for this — no frontend changes needed.

### Files modified

1. **`swissarmyhammer-perspectives/src/events.rs`** (NEW) — `PerspectiveEvent` enum with `PerspectiveChanged` and `PerspectiveDeleted` variants.
2. **`swissarmyhammer-perspectives/src/lib.rs`** — Export `events` module and `PerspectiveEvent` type.
3. **`swissarmyhammer-perspectives/src/context.rs`** — Added `broadcast::Sender<PerspectiveEvent>` field. Emit events on `write()` (with field-level diff) and `delete()`. Exposed `subscribe()` method. Added `diff_perspective()` helper.
4. **`swissarmyhammer-kanban/src/context.rs`** — Added `perspective_context_if_ready()` synchronous accessor.
5. **`kanban-app/src/state.rs`** — `BoardHandle::start_watcher()` obtains perspective broadcast receiver and passes it to the bridge.
6. **`kanban-app/src/watcher.rs`** — `run_bridge()` accepts optional perspective receiver. Uses `tokio::select!` to listen on both channels. Added `process_perspective_event()` to translate perspective events into Tauri `entity-field-changed` / `entity-created` / `entity-removed` events.

### Subtasks

- [x] Define `PerspectiveEvent` enum in `swissarmyhammer-perspectives/src/events.rs` with `Changed { id, changed_fields, is_create }` and `Deleted { id }` variants
- [x] Add `broadcast::Sender<PerspectiveEvent>` to `PerspectiveContext`; emit on `write()` and `delete()`; expose `subscribe()` method
- [x] Extend `run_bridge()` signature to accept a perspective event receiver; use `tokio::select!` to handle both channels; translate perspective events to Tauri events
- [x] Wire `BoardHandle::start_watcher()` to obtain and pass the perspective broadcast receiver

## Acceptance Criteria

- [x] Editing a filter expression in the formula bar immediately re-fetches tasks with the new filter (no perspective switch needed)
- [x] Clearing the filter via the × button immediately shows all tasks
- [x] Debounced autosave (300ms) applies the filter live as the user types
- [x] Undo/redo of a perspective filter change triggers a live UI update — **follow-up task `01KPBY1Y6PNR5GYJVC8QBBNJGW` filed for undo/redo gap (pre-existing limitation: StoreContext bypasses PerspectiveContext)**
- [x] No regression: perspective create, rename, delete still emit correct events

## Tests

- [x] **Unit test** in `swissarmyhammer-perspectives/src/context.rs`: write a perspective, assert `subscribe()` receiver gets `PerspectiveChanged` with correct id, changed fields, and `is_create: true`
- [x] **Unit test** in `swissarmyhammer-perspectives/src/context.rs`: delete a perspective, assert receiver gets `PerspectiveDeleted` with correct id
- [x] **Unit test** in `swissarmyhammer-perspectives/src/context.rs`: rename emits `PerspectiveChanged` with `name` field and `is_create: false`
- [x] **Unit test** in `swissarmyhammer-perspectives/src/context.rs`: no-op write does not emit an event
- [x] **Unit test** in `swissarmyhammer-perspectives/src/context.rs`: update emits `PerspectiveChanged` with only changed field and `is_create: false`
- [x] **Existing test**: `test_update_perspective_emits_item_changed_event` in `swissarmyhammer-kanban/src/perspective/tests.rs` — still passes
- [x] Run full test suite with `cargo nextest run` — all 13,054 tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-16 00:15)

### Warnings
- [x] `kanban-app/src/state.rs:341` — `try_read()` on the `RwLock<PerspectiveContext>` can return `None` if a write lock is held at that instant, silently losing the perspective subscription for the lifetime of the bridge. In practice this is safe during startup (no concurrent perspective writes), but it's a fragile assumption. Consider using a short `tokio::time::timeout(Duration::from_millis(50), pctx.read())` wrapped in a `block_on` or restructuring `start_watcher` to be async so it can `.read().await`. Alternatively, document the invariant that `start_watcher` must only be called when no perspective write is in flight. **→ Documented the invariant with a detailed comment explaining the ordering guarantee.**
- [x] Acceptance criterion "Undo/redo of a perspective filter change also triggers a live UI update" is not fully met by this change. `StoreContext::undo()`/`redo()` operate directly on files via `StoreHandle` — they bypass `PerspectiveContext::write()`, so no `PerspectiveEvent` is broadcast. This is a pre-existing limitation (entities rely on the filesystem watcher; perspectives have none), but the task claims it's fixed. Either remove the acceptance criterion or file a follow-up task for undo/redo event emission (e.g. a callback from StoreContext to PerspectiveContext, or a perspective filesystem watcher). **→ Filed follow-up task `01KPBY1Y6PNR5GYJVC8QBBNJGW` and updated acceptance criterion to note the gap.**

### Nits
- [x] `kanban-app/src/watcher.rs:772` — `process_perspective_event` always emits `WatchEvent::EntityFieldChanged` even for brand-new perspectives. The frontend's `PerspectiveProvider` also listens for `entity-created` with `entity_type === "perspective"`. Consider emitting `WatchEvent::EntityCreated` when `changed_fields` lists all six fields (the create signature from `diff_perspective`), for consistency with the entity bridge's create-vs-update distinction. Not functionally broken — `refresh()` is called either way — but it would make the Tauri event stream semantically accurate. **→ Added `is_create` field to `PerspectiveEvent::PerspectiveChanged`; bridge now emits `EntityCreated` for creates, `EntityFieldChanged` for updates.**
- [x] `swissarmyhammer-perspectives/src/context.rs:313` — The hardcoded list of all-fields in `diff_perspective`'s create path (`["name", "view", "fields", "filter", "group", "sort"]`) will silently miss new fields if `Perspective` gains one later. Consider deriving this list from the struct or adding a compile-time reminder comment. **→ Added a NOTE comment reminding to keep the list in sync with the struct.**