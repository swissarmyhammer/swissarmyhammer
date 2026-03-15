---
position_column: done
position_ordinal: v5
title: 'CODE-CONTEXT-WATCHER: Implement file watcher with fanout callback'
---
Integrate file watcher with fanout to multiple handlers per spec lines 175-189.

**Requirements:**
- One WorkspaceWatcher with FanoutWatcherCallback
- Fanout handlers:
  - TsWatcherHandler: triggers tree-sitter re-index
  - LspWatcherHandler: notifies LSP server, marks dirty
- On file change: immediately write to DB, clear ts_indexed/lsp_indexed flags
- Dirty state is durable in DB (not just in-memory)
- Graceful degradation if watcher fails
- Support adding new handlers in future (extensible)

**Quality Test Criteria:**
1. Build succeeds
2. Unit test: FanoutWatcherCallback distributes events to all handlers
3. Integration test on real project:
   - Modify a file → ts_indexed flag cleared in DB within 500ms
   - Modify a file → lsp_indexed flag cleared in DB within 500ms
   - Delete a file → marked for removal
   - Create a file → added to indexed_files
   - Process crash during watch → next startup sees dirty flags
   - Multiple file changes batched correctly (debounced)