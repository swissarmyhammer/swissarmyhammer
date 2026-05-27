---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8f80
title: FSEvents stream REGISTRATION (~4.5s) on background startup blocks runtime teardown in in-process MCP server tests
---
## What

Follow-up to 01KS662VB1ARQ80YGAP1PS7E65 (teardown-detach + shutdown lock-contention fix). Those fixes are DONE and verified: in-process MCP server `shutdown()` dropped from ~5s to ~20us, and `FileWatcher::start` now builds/registers OFF the shared lock so shutdown never blocks on an in-flight startup.

**Remaining, DISTINCT facet (profiled 2026-05-21, hard data below):** the in-process MCP server tests (`mcp::test_utils::tests::test_client_list_tools`, `test_client_call_tool`, etc.) are STILL ~4.5s each — but NOT in any measured code phase. Profiling a single `test_client_list_tools`:

```
PROBE start_mcp_server   = 19.1 ms
PROBE create_test_client = 7.2  ms   (handshake)
PROBE list_tools         = 9.7  ms
PROBE client.cancel      = 0.5  ms
PROBE server.shutdown    = 21   us   <-- FIXED by 01KS662VB1ARQ80YGAP1PS7E65
PROBE FileWatcher::start = 4.55 s    <-- the remaining cost
PASS  test wall time     = 4.59 s
```

### Diagnosis
- The ~4.5s is the macOS FSEvents stream **REGISTRATION** — `debouncer.watcher().watch(path, RecursiveMode::Recursive)` inside `FileWatcher::start`. This is a synchronous, blocking OS call that serializes through the FSEvents subsystem.
- It runs on a background `tokio::spawn` task started during the MCP `initialize` handler (`spawn_background_file_watcher` -> `start_file_watching` -> `FileWatcher::start`). The handshake does NOT wait for it (correct).
- `server.shutdown()` sets `file_watch_stopped` and `abort()`s the startup task — but `abort()` cannot interrupt a synchronous in-flight `.watch()`. The tokio worker thread stays stuck in `.watch()` until it returns (~4.5s).
- When the test fn returns and the `#[tokio::test(flavor = "multi_thread")]` runtime is dropped, runtime teardown waits for that blocked worker -> the test's wall time is ~4.5s.
- Under full-workspace load (~32 parallel FSEvents registrations) this serializes and is the dominant remaining cost; it is a THIRD facet, distinct from (a) teardown drop [fixed] and (b) shutdown lock-contention [fixed].

### Likely fix (needs design sign-off — architectural change beyond "fix shutdown now")
Run the blocking `.watch()` registration on a path that does NOT pin a tokio worker, so an aborted/abandoned startup cannot block runtime teardown. Options:
1. Perform the FSEvents `.watch()` on a dedicated detached `std::thread` (like the teardown detach), delivering the started debouncer back via a channel; the tokio task only awaits the channel and is cleanly abortable. A late delivery after shutdown is dropped (honor `file_watch_stopped`).
2. Use `tokio::task::spawn_blocking` for the `.watch()` so it does not occupy a core worker — but spawn_blocking threads also block runtime shutdown unless abandoned; verify behavior.
3. Skip starting the file watcher entirely when there is no client that needs prompt hot-reload (e.g., gate on a capability/flag) — but this changes behavior, weigh carefully.

Preserve hot-reload behavior; bounded thread usage; no serial guards / nextest slow-timeout overrides; WHY doc comments.

## Acceptance Criteria
- [ ] In-process MCP server tests (`test_client_list_tools`, `test_client_call_tool`, `test_mcp_server_*`) complete FAST (well under 1s each in isolation) — no ~4.5s tail from FSEvents registration blocking runtime teardown.
- [ ] FSEvents `.watch()` registration no longer pins a tokio worker that blocks runtime/test teardown.
- [ ] Hot-reload still works (prompt file change -> reload -> notification) — existing event tests pass.
- [ ] `cargo nextest run --workspace` FSEvents cluster passes fast under full load (3 consecutive runs, early-bail on timeout).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean.
- [ ] No serial guards or nextest slow-timeout overrides.

## Tests
- [ ] Guard test that an in-process server's full lifecycle (start -> handshake -> shutdown -> drop) completes in well under 1s, even with file watching enabled.

## Workflow
- Use `/tdd`. Measure before/after the in-process server test wall time. #test-failure