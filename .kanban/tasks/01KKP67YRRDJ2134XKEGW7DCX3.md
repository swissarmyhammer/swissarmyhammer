---
depends_on:
- 01KKP67N3S967JCAJGNG7CVA3H
position_column: done
position_ordinal: '9e80'
title: Mount EntitySearchIndex in BoardHandle and sync with entity events
---
## What
Add `RwLock<EntitySearchIndex>` to `BoardHandle` and keep it in sync with entity changes — both from our own command writes (via `flush_and_emit`) and from external file changes (via the watcher).

**Files:**
- `kanban-app/src/state.rs` — add `search_index: RwLock<EntitySearchIndex>` to BoardHandle
- `kanban-app/src/commands.rs` — after `flush_and_emit`, update search index for changed entities
- `kanban-app/src/watcher.rs` — in watcher callback, update search index for external changes
- `kanban-app/Cargo.toml` — add `swissarmyhammer-entity-search` dependency

**Approach:**
- On `BoardHandle::open()`: load all entities from EntityContext across all types, create `EntitySearchIndex::from_entities(all_entities)`
- After `flush_and_emit()` in `dispatch_command`: for each WatchEvent, reconstruct Entity and call `search_index.write().update()` or `.remove()`
- In watcher callback: same — update/remove based on resolved events
- Embedding rebuild is async and non-blocking — triggered after bulk load, then lazy on changes via stale tracking

## Acceptance Criteria
- [ ] Search index populated on board open with all entity types
- [ ] Own writes (dispatch_command) update search index immediately
- [ ] External file changes (watcher) update search index
- [ ] Entity removal clears from search index
- [ ] No blocking on embedding rebuild

## Tests
- [ ] Integration test: open board, search returns entities
- [ ] `cargo nextest run -p kanban-app`