---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9580
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

---

## Cross-reference from 01KS5XY57HWP9FP7M72Y6A4749 (2026-05-21) — this FSEvents issue is BROADER than these tests

While fixing the in-process-MCP-server HTTP-handshake timeouts (task 01KS5XY57HWP9FP7M72Y6A4749), I found the notify/FSEvents `AsyncDebouncer` is also the dominant remaining cost for the WHOLE in-process-MCP-server test cluster, not just `mcp::file_watcher::tests`:

- Every in-process MCP server (`start_mcp_server_with_options`) starts a prompt-directory file watcher on startup (`McpServer::spawn_background_file_watcher` → `FileWatcher::start_watching` → `AsyncDebouncer::new_with_channel`, FSEvents on macOS).
- Phase timing for one server test (after the handshake was fixed to ~6ms): server start 23ms, handshake 6ms, client cancel 0ms, **server shutdown 5057ms**. The 5s is `McpServerHandle::shutdown` → `stop_file_watching()` dropping the `AsyncDebouncer`.
- Under full-workspace nextest, creating/dropping many FSEvents debouncers concurrently serializes through the OS FSEvents subsystem — the same cross-process bottleneck class as the macOS `configd` proxy issue. This is why the MCP-server cluster (cli `mcp_integration` trio, tools `test_utils` client tests, kanban-app open-board tests) STILL times out under full load even though the handshake is now fast and they all pass in isolation (8/8 in 25s).

**Implication for the fix here**: a fix that only serializes `mcp::file_watcher::tests` will NOT resolve the broader cluster. Consider a root-cause fix at the source, e.g. one of:
- Make `FileWatcher::stop_watching` / debouncer drop non-blocking (the 5s drop is the per-test killer), or
- Skip starting the prompt file watcher for short-lived in-process servers used in tests (a startup option), or
- A shared/process-wide debouncer instead of one per server instance.

Such a source fix would also unblock the full-workspace 3x-fast acceptance criterion of 01KS5XY57HWP9FP7M72Y6A4749. Related sibling tasks: 01KS5B9ZH604586D1AHC3NVPFJ (swissarmyhammer-entity watcher::tests), 01KS57J5F5YJ3QSB34Q139221R / 01KS57J8PPWDF20CQ9R3ES8XRP / 01KS3TQJ6KT96XH5TR5PY84GC8 (the MCP-server-startup cluster).