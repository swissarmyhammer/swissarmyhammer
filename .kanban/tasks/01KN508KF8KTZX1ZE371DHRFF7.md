---
assignees:
- claude-code
depends_on:
- 01KN5083QR3K060WAK60YEFAB9
position_column: done
position_ordinal: fffffffffffffffffffff980
title: Route file watcher through StoreContext
---
## What

Replace the entity-specific file watcher with generic routing through `StoreContext.store_for_path()` → `flush_changes()`. When the watcher detects a file change, it finds the owning store and lets it produce change events.

**Files to modify:**
- `kanban-app/src/` — wherever the file watcher is set up (likely `state.rs` or a watcher module)
- The watcher callback currently calls entity-specific change detection. Replace with:
  ```rust
  if let Some(store) = store_context.store_for_path(&changed_path).await {
      let events = store.flush_changes().await;
      for event in events {
          app.emit(&event.event_name, &event.payload);
      }
  }
  ```

**Approach:**
- The file watcher already watches `.kanban/` recursively
- Currently routes to entity-specific cache refresh
- New: match changed path against registered stores via `store_for_path()`
- Each store's `flush_changes()` diffs files vs its cache and produces `ChangeEvent`s
- Events emitted to frontend — same as the command path

**What this enables:**
- External edits to perspective files (`.kanban/perspectives/*.yaml`) are automatically detected
- External edits to entity files (`.kanban/tasks/*.md`) continue to work
- No watcher-specific code per store type — all generic through the trait

## Acceptance Criteria
- [ ] File watcher routes to store via `store_for_path()`
- [ ] External entity file edit → UI updates (same as before)
- [ ] External perspective file edit → UI updates (new capability)
- [ ] No entity-specific watcher code remains
- [ ] All existing tests pass

## Tests
- [ ] `cargo nextest run --workspace` — all pass
- [ ] Manual: edit a task .md file externally → UI reflects change
- [ ] Manual: edit a perspective .yaml file externally → UI reflects change