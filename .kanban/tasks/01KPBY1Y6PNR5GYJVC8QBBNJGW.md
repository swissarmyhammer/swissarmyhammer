---
assignees:
- claude-code
position_column: todo
position_ordinal: 8a80
title: Perspective undo/redo must emit events to refresh the UI
---
## What

`StoreContext::undo()`/`redo()` for perspectives operate directly on files via `StoreHandle` — they bypass `PerspectiveContext::write()`, so no `PerspectiveEvent` is broadcast and the UI doesn't refresh. This is a pre-existing limitation: entities rely on the filesystem watcher to detect undo/redo file changes, but perspectives have no filesystem watcher.

The forward path (filter edit, clear, create, delete, rename) is fixed by the broadcast channel in `PerspectiveContext`. This task covers the undo/redo gap specifically.

### Possible approaches

1. **Callback from StoreContext**: After undo/redo completes, call back into `PerspectiveContext` to emit an event and sync the in-memory cache.
2. **Perspective filesystem watcher**: Watch the perspectives directory for changes (like the entity watcher does).
3. **Post-command refresh in dispatch layer**: After `app.undo`/`app.redo` commands, emit a perspective refresh event from `flush_and_sync_after_command`.

### Files likely involved

- `swissarmyhammer-store/src/context.rs` — undo/redo methods
- `kanban-app/src/commands.rs` — `flush_and_sync_after_command`
- `swissarmyhammer-perspectives/src/context.rs` — in-memory cache sync

## Acceptance Criteria
- [ ] Undoing a filter change immediately refreshes the task list with the restored filter
- [ ] Redoing a filter change immediately applies the filter again
- [ ] PerspectiveContext in-memory cache stays in sync after undo/redo

## Tests
- [ ] Integration test: write perspective, undo, verify event emitted and cache updated
- [ ] Integration test: redo after undo, verify event emitted
- [ ] `cargo nextest run` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.