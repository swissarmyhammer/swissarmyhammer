---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9d80
title: 'Fix: flush_and_emit returns 0 events after task.move (drag-drop snap-back)'
---
## Bug

Dragging a card to reorder within a column succeeds on the backend (task.move writes correct ordinal to disk) but the UI snaps the card back to its original position. The frontend never receives an `entity-field-changed` event for the position change.


**IMPORANT**

You must fix an test this without askign the user to do any work.

## What's proven (5 tests pass)

1. **Ordinal logic correct** — `reorder_move_third_before_second` 
2. **Disk write correct** — `task_move_writes_new_ordinal_to_disk` confirms .md file gets new ordinal
3. **Registry correct** — `task_move_is_undoable_in_registry` confirms undoable=true
4. **Watcher detects changes** — `test_flush_and_emit_detects_task_position_ordinal_change` confirms flush_and_emit CAN detect ordinal changes in .md files
5. **Runtime dispatch correct** — diagnostic logging confirms `flush_and_emit_for_handle` IS called with `undoable=true, has_handle=true`

## Where the bug is

`flush_and_emit_for_handle` (kanban-app/src/commands.rs:1151) calls `flush_and_emit` which returns 0 events at runtime, even though the .md file on disk has changed. The unit test proves the function works in isolation, so the issue is a **cache state mismatch** at runtime.

### Most likely root cause: path canonicalization

`flush_and_emit` canonicalizes paths (line 196, 209) and compares them against the cache. The cache is built at board open time via `new_entity_cache` which also canonicalizes. If the `KanbanContext.root()` path and the cache's paths use different symlink resolutions (e.g. `/var` vs `/private/var` on macOS), `flush_and_emit` would scan files under one canonical path but the cache keys use another — resulting in all files appearing as "new" on first call, then all matching on subsequent calls (after being inserted with the scan's canonical path).

### How to verify

Add `tracing::info!` inside `flush_and_emit` that logs the number of disk_paths found, cache entries, and any mismatches. Or write a test that creates a symlinked kanban root and checks if flush_and_emit detects changes correctly.

### Fix approach

Either:
1. Normalize the kanban_root path in `flush_and_emit` to match the cache key format
2. Or have `flush_and_emit_for_handle` pass the same root that was used to build the cache

## Acceptance Criteria
- [ ] Dragging a card to reorder within a column updates the UI without a manual refresh
- [ ] The `entity-field-changed` event fires with the new `position_ordinal` value
- [ ] Add a test: call flush_and_emit after a simulated task.move with the SAME cache instance, verify events > 0 #smoke-test