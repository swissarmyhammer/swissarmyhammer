---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: 'Fix flaky timeout: test_file_watcher_start_watching_replaces_previous'
---
**File**: `crates/swissarmyhammer-tools/src/mcp/file_watcher.rs` (inside `mcp::file_watcher::tests`)

**Symptom**: Times out at 300s under full workspace nextest, but PASS [8.3s] in isolation.

**Root cause hypothesis**: `notify`-based file-watcher test under heavy filesystem pressure from 8000+ parallel tests. Likely waiting on a filesystem event that never arrives because tmp-dir reuse or watcher-handle collision under load.

**Reproducer**:
- `cargo nextest run --workspace` -> TIMEOUT [300s]
- `cargo nextest run -E 'test(test_file_watcher_start_watching_replaces_previous)'` -> PASS [8.3s]

**Suggested fix**: Ensure each test uses a unique `tempfile::TempDir`; consider serializing file_watcher tests with `#[serial_test::serial]`; add deadline/timeout to the watcher event wait so the test fails fast with a clear diagnostic.

**Acceptance criteria**: 3 consecutive `cargo nextest run --workspace` runs complete with this test passing.

**Pre-existing**: file unchanged from `main`. Not caused by recent UI work on the `kanban` branch.

#test-failure