---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: 'Fix flaky timeout: swissarmyhammer-entity watcher::tests (parallel contention)'
---
**File**: `crates/swissarmyhammer-entity` (inside `watcher::tests`)

**Tests timing out** (observed 2026-05-21 during full-workspace `cargo nextest run --workspace`):
- `watcher::tests::entity_watcher_drop_sends_shutdown`
- `watcher::tests::test_attachment_create_emits_event`
- `watcher::tests::test_attachment_remove_emits_event`

**Symptom**: All time out at 300s under full-workspace nextest. Likely pass in isolation.

**Root cause hypothesis**: Same in-process-server / file-watcher / event-driven contention family as the already-tracked `mcp::file_watcher` and `kanban-app` open-board timeouts. Under ~13.7k-test parallel load these event-driven watcher tests starve waiting on an event/shutdown signal that never arrives in time.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s] for all three
- Confirm in isolation: `cargo nextest run -p swissarmyhammer-entity -E 'test(entity_watcher_drop_sends_shutdown) | test(test_attachment_create_emits_event) | test(test_attachment_remove_emits_event)'`

**Suggested fix**: Follow the established pattern — serialize the watcher tests with `#[serial_test::serial(<group>)]` and/or add a per-test nextest `slow-timeout` override in `.config/nextest.toml` scoped via a `test(...)` filter (see the `mcp_server` override added for the mcp_integration tests as a reference). Do NOT silence with `--test-threads=1`.

**Acceptance criteria**: full-workspace `cargo nextest run --workspace` completes with all three tests passing.

**Pre-existing**: same contention class as sibling test-failure tasks. Discovered during full-workspace verification of task 01KS3TQJ6KT96XH5TR5PY84GC8.

#test-failure