---
assignees:
- claude-code
depends_on:
- 01KN79CSWG20HKJ9W86CTQ68XE
position_column: done
position_ordinal: ffffffffffffffffffffffab80
title: 2. Replace WATCHED_SUBDIRS with store-driven watching — stores watch themselves
---
## What

The file watcher has a hardcoded `WATCHED_SUBDIRS` list in `watcher.rs:21-29`. This is wrong — the store knows its own directory via `root()`. The watcher should watch whatever directories the registered stores manage. When a store is registered in `StoreContext`, its directory gets watched automatically.

### Current problem
- `WATCHED_SUBDIRS = ["tasks", "tags", "columns", "swimlanes", "actors", "boards", "views"]`
- `"perspectives"` is missing — so perspective file changes are invisible
- Adding to the hardcoded list is the wrong fix — we'd have to update it every time a new store type is added

### Fix approach
1. Remove `WATCHED_SUBDIRS` from `watcher.rs`
2. `start_watching()` takes a list of directories to watch (from StoreContext's registered stores)
3. `StoreContext` exposes `watched_roots()` returning all store `root()` paths
4. `BoardHandle::start_watcher()` in `state.rs` calls `store_context.watched_roots()` to get the directory list
5. `new_entity_cache()` also uses the store roots instead of the hardcoded list
6. `flush_and_emit()` scans store roots instead of hardcoded list
7. `path_to_entity()` derives entity_type from directory name (already works — strips trailing 's')

This way perspectives (and any future store types) are automatically watched when registered.

### Files to modify
- `swissarmyhammer-store/src/context.rs` — add `watched_roots()` method
- `swissarmyhammer-store/src/erased.rs` — ensure `root()` is on `ErasedStore` (it already is)
- `kanban-app/src/watcher.rs` — remove `WATCHED_SUBDIRS`, change `start_watching()`, `new_entity_cache()`, `flush_and_emit()` to accept directory list
- `kanban-app/src/state.rs` — pass store roots to watcher functions

## Acceptance Criteria
- [ ] `WATCHED_SUBDIRS` constant removed
- [ ] Watcher watches directories from registered stores
- [ ] Perspective files automatically watched (no special code)
- [ ] Entity files still watched (same behavior)
- [ ] Adding a new store type automatically watches its directory
- [ ] All existing watcher tests pass (updated to use dynamic roots)

## Tests
- [ ] Existing watcher tests updated to use store-provided roots
- [ ] New test: perspective store registered → perspective directory watched → file change detected
- [ ] `cargo nextest run -p kanban-app` — all pass
- [ ] `cargo nextest run --workspace` — no regressions
- [ ] Manual: edit perspective .yaml externally → watcher fires event