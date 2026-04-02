---
assignees:
- claude-code
position_column: todo
position_ordinal: '9780'
title: Remove StoreHandle cache — disk is the source of truth
---
## What

StoreHandle maintains an in-memory `cache: RwLock<HashMap<String, String>>` that duplicates file content. This creates a dual-cache problem with the watcher's EntityCache. The file on disk is the source of truth — the store should always read from disk.

## Changes

### Remove from StoreHandle
- Remove `cache` field from `StoreHandle` struct
- `write()`: don't update cache, just read old content from disk before writing
- `delete()`: don't remove from cache
- `flush_changes()`: compare against disk state directly (or remove entirely — the watcher handles external change detection)
- `has_entry()`: no cache dependency
- `undo()`/`redo()`: read from disk, not cache

### Impact on flush_changes()
Without a cache, `flush_changes()` can't diff "current vs last known." Two options:
1. Remove `flush_changes()` from StoreHandle entirely — let the watcher (EntityCache) handle all change detection
2. Keep flush_changes but have it always read from disk — it becomes a "read all items" operation

Option 1 is simpler. The watcher already handles change detection with content hashing. StoreHandle focuses on write/delete/undo/redo.

### Impact on write() idempotent check
Currently `write()` compares new text against cached text to skip no-op writes. Without cache, it reads from disk each time. This is fine — disk reads are cheap and correct.

## Acceptance Criteria
- [ ] `cache` field removed from StoreHandle
- [ ] write() reads old content from disk (not cache)
- [ ] No dual-cache problem
- [ ] All tests pass
- [ ] flush_changes() either removed or reads from disk

## Tests
- [ ] `cargo nextest run --workspace` — all pass