---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffdf80
title: 'Bug: Task drag-and-drop does not visually update the board'
---
## What

Dragging a task card to a new column or position does not produce any visible change on the board. The backend `task.move` command executes successfully (the task file on disk is updated), but the frontend never re-renders the task in its new position.

### Root cause analysis

The event pipeline from backend write to frontend state update has a gap in `kanban-app/src/commands.rs` in `flush_and_emit_for_handle` (line ~1416).

Two event sources exist:
1. **Store events** (line 1420): `store_context.flush_all()` — returns `ChangeEvent`s for entities managed by stores
2. **Watcher events** (line 1430): `flush_and_emit()` — detects filesystem changes by diffing file hashes

The store events are converted to `WatchEvent::EntityFieldChanged` at line 1464-1469 with `changes: vec![]` and `fields: None`. When the frontend receives this, it falls through to the re-fetch path (line 319-341 in `rust-engine-container.tsx`) which calls `get_entity`. This path should work.

**Likely problem**: `store_context.flush_all()` may not be returning events for task moves. `EntityContext.write()` delegates to `StoreHandle.write()` which should emit `item-changed`, but the flush mechanism may not be collecting pending events. Alternatively, the event pipeline may be discarding watcher-detected changes at line 1482-1487 (`// Keep only attachment events from the watcher`) before the store path has a chance to detect them.

### Files to investigate and fix

1. **`kanban-app/src/commands.rs`** (`flush_and_emit_for_handle`, line ~1416)
   - Verify that `store_context.flush_all()` returns events after `task.move` writes via `EntityContext`
   - If store events are empty, the watcher events at line 1482 are the fallback but they're discarded for non-attachment types
   - Fix: for entity types where the store doesn't report changes, allow watcher events through (don't filter to attachments only)

2. **`swissarmyhammer-store/src/context.rs`** (`flush_all`)
   - Check that `flush_all()` properly collects pending `ChangeEvent`s from all registered stores after a write

3. **`kanban-app/src/commands.rs`** (store event conversion, line ~1464)
   - The conversion creates `EntityFieldChanged` with empty `changes` and `None` fields — the frontend must re-fetch. Verify this re-fetch path works by adding the full `fields` to the event (read entity from EntityContext after write)

### Debugging approach

Add `tracing::info!` logging to `flush_and_emit_for_handle` to log:
- Number of store events returned by `flush_all()`
- Number of watcher events returned by `flush_and_emit()`
- Number of events after filtering (line 1487)
- Each emitted event name, entity_type, and id

Then reproduce the bug and check `log show --predicate 'subsystem == \"com.swissarmyhammer.kanban\"'` output.

## Acceptance Criteria

- [ ] Dragging a task card to a different column shows the task in the new column immediately (within ~200ms)
- [ ] Dragging a task card to reorder within a column shows the task in the new position
- [ ] The event pipeline emits `entity-field-changed` with actionable data after `task.move`
- [ ] `cargo test -p kanban-app` passes
- [ ] `cd kanban-app/ui && npx vitest run` passes

## Tests

- [ ] `kanban-app/src/commands.rs` — add test: `dispatch_command_internal` for `task.move` produces at least one `entity-field-changed` event
- [ ] `kanban-app/src/commands.rs` — add test: `flush_and_emit_for_handle` does not discard entity events when store reports changes
- [ ] Run `cargo test -p kanban-app` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.