---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Wire update_cache into command execution to suppress self-write watcher events
---
## What

`watcher::update_cache()` (`kanban-app/src/watcher.rs:217`) is a public function designed to update the entity cache immediately after command-driven writes, so the file watcher's debounce doesn't treat our own writes as external changes. It has tests and a clear docstring, but **no production caller** — only test code calls it (lines 1270, 1996, 2731).

The `EntityCache` is already available on `BoardHandle` (`kanban-app/src/state.rs:78`) and the watcher's debounce logic already checks cached hashes (line 452). The missing piece is calling `update_cache` from the write paths in command execution.

### Current flow (broken)
1. Command writes entity file to disk
2. Watcher detects filesystem change after debounce
3. Hash comparison may or may not match depending on timing — race condition
4. `flush_and_emit` (`commands.rs:1511`) is called synchronously as a workaround, but the async watcher can still fire spuriously

### Desired flow
1. Command writes entity file to disk
2. Command calls `update_cache(&entity_cache, &written_path)` immediately
3. Watcher debounce fires, compares hash, finds no change → no spurious event

### Files to modify
- `kanban-app/src/commands.rs` — Call `watcher::update_cache()` after entity file writes. Identify all write paths that go through `StoreContext` or direct file writes and add cache updates.
- `kanban-app/src/state.rs` — May need a helper method on `BoardHandle` to expose `update_cache` ergonomically (e.g., `handle.update_entity_cache(path)`)

### Approach
1. Audit write paths in `commands.rs` to identify where entity files are persisted
2. Add `update_cache` calls after each write, using `handle.entity_cache`
3. Verify the compiler warning disappears
4. Confirm no duplicate watcher events fire in integration tests

## Acceptance Criteria
- [ ] `cargo build 2>&1 | grep "update_cache"` produces no warnings
- [ ] `update_cache` is called from at least one production code path after entity file writes
- [ ] Existing `test_update_cache_suppresses_change_detection` and `test_own_write_suppressed` tests continue to pass
- [ ] No new spurious watcher events in integration tests

## Tests
- [ ] Existing tests in `kanban-app/src/watcher.rs` — `test_update_cache_suppresses_change_detection` (line 1986) and `test_own_write_suppressed` (line 2706) must still pass
- [ ] Add integration test in `watcher.rs`: command write → update_cache → verify no `WatchEvent` emitted via `flush_and_emit`
- [ ] `cargo nextest run -p kanban-app` — all tests pass, zero warnings

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.