---
assignees:
- claude-code
position_column: todo
position_ordinal: '9680'
title: 'E2E test: external file changes propagate events to UI'
---
## What

Verify end-to-end that when `.kanban/` entity files are modified externally (e.g. by another process, manual edit, or MCP tool), the file watcher detects changes and the UI receives the correct Tauri events to update its state.

The event pipeline is:
1. **File change on disk** → `notify` crate fires raw fs events
2. **`kanban-app/src/watcher.rs`** → `classify_event()` filters entity files, debounces, `resolve_change()` compares SHA-256 hashes against `EntityCache`, diffs fields → emits `WatchEvent`
3. **`kanban-app/src/state.rs:227`** → `start_watcher()` callback receives `WatchEvent`, calls `app_handle.emit()` with event names `entity-created`, `entity-removed`, `entity-field-changed`, `attachment-changed`
4. **`kanban-app/ui/src/App.tsx:296-401`** → `useEffect` listeners receive events, patch local React state via `setEntitiesFor()`

### Files to modify/create
- `kanban-app/src/watcher.rs` — add integration tests that verify the full `start_watching → external write → callback fires` path for all event types
- `kanban-app/ui/src/App.test.tsx` or `kanban-app/ui/src/lib/entity-event-propagation.test.tsx` — add frontend tests that simulate Tauri events and verify React state updates

### Approach
- **Rust side**: Existing unit tests in `watcher.rs` (lines 759+) cover `flush_and_emit`, `classify_event`, hash diffing, and `start_watching` for creates/modifies/removes. Gaps: no test covers field-level diffing through `start_watching` (only through `flush_and_emit`), and no test verifies the `EntityCache` suppression works across the full async pipeline (watcher debounce + hash comparison).
- **Frontend side**: Only `undo-context.test.tsx` checks that listeners are registered. No test verifies that a simulated `entity-field-changed` event actually updates the entity store, or that `entity-created` adds to the list, or that `entity-removed` removes from it.

## Acceptance Criteria
- [ ] Rust integration test: write a YAML entity file externally while `start_watching` is running → callback fires `EntityFieldChanged` with correct `changes` vec containing the modified field names and values
- [ ] Rust integration test: create a new YAML entity file externally → callback fires `EntityCreated` with correct `entity_type`, `id`, and `fields`
- [ ] Rust integration test: delete a YAML entity file externally → callback fires `EntityRemoved` with correct `entity_type` and `id`
- [ ] Rust integration test: write a file from "our own" code path (update cache first) → watcher does NOT fire (suppression works)
- [ ] Frontend test: simulate `entity-field-changed` Tauri event → verify entity store state is updated with new field values
- [ ] Frontend test: simulate `entity-created` Tauri event → verify new entity appears in the store
- [ ] Frontend test: simulate `entity-removed` Tauri event → verify entity is removed from the store

## Tests
- [ ] `cargo test -p kanban-app watcher::tests::test_start_watching_field_change_event` — new test, should pass
- [ ] `cargo test -p kanban-app watcher::tests::test_start_watching_creates_entity_event` — new test, should pass
- [ ] `cargo test -p kanban-app watcher::tests::test_start_watching_removes_entity_event` — new test, should pass
- [ ] `cargo test -p kanban-app watcher::tests::test_start_watching_suppresses_own_writes` — new test, should pass
- [ ] `cd kanban-app/ui && npx vitest run entity-event-propagation` — new frontend test file, all pass
- [ ] `cargo test -p kanban-app` — all existing watcher tests still pass (no regressions)