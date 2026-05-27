---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9080
title: Make notify file-watcher teardown non-blocking (FileWatcher + EntityWatcher) to fix FSEvents-contention timeouts
---
## What

**Root cause (profiled 2026-05-21)** of the remaining in-process-server / watcher test timeouts under full-workspace `cargo nextest run`: dropping a `notify` file-watcher blocks for ~5s on macOS FSEvents stream teardown, and that teardown serializes through the OS FSEvents subsystem — so under load (~10+ watchers torn down concurrently) it queues and crosses the 300s nextest kill. This is the SAME OS-subsystem-serialization class as the already-fixed `configd` proxy issue. It is the dominant remaining cost after the proxy + tokio-runtime fixes (commit `62f5d065a`): each in-process MCP server's `shutdown()` spends ~5057ms in `stop_file_watching()` dropping the `AsyncDebouncer`.

Two watchers share the pattern:
- `crates/swissarmyhammer-tools/src/mcp/file_watcher.rs` — `FileWatcher` holds `debouncer: Option<AsyncDebouncer<RecommendedWatcher>>`. `stop_watching()` (called by `stop_file_watching()` and by `Drop`) does `self.debouncer.take()` and drops it synchronously on the caller's thread.
- `crates/swissarmyhammer-entity/src/watcher.rs` — `EntityWatcher` holds `_watcher: RecommendedWatcher`; its `Drop` sends a shutdown oneshot but the `_watcher` field then drops synchronously, blocking on FSEvents teardown.

## Fix

Detach the blocking notify-watcher drop to a background OS thread so the caller's shutdown/Drop returns immediately while the OS teardown completes off the critical path. This preserves the hot-reload / entity-watch feature entirely — only the teardown timing moves off-path.

- `FileWatcher::stop_watching`: when taking the debouncer, move it into `std::thread::spawn(move || drop(debouncer))` instead of dropping inline. DONE.
- `EntityWatcher::drop`: send shutdown signal, then move `_watcher` (now `Option<RecommendedWatcher>`) into `std::thread::spawn(move || drop(watcher))`. DONE.

### Second facet (lock contention) — FIXED 2026-05-21 (option 1 + non-blocking shutdown)
Teardown detach alone did NOT make `shutdown()` fast: profiling showed `shutdown()` still ~5s, spent waiting on `self.file_watcher.lock().await` because the BACKGROUND `start_file_watching` task held that lock across the slow FSEvents `.watch()` registration. Fixed per approved direction:
- `FileWatcher::start(callback)` — new associated constructor that builds the debouncer + performs the slow `.watch()` registration WITHOUT `&mut self` / without any shared lock, returning a started `FileWatcher`. `start_watching` is now a thin `&mut self` wrapper over it.
- `McpServer::start_file_watching_with_callback` builds via `FileWatcher::start` OFF the `file_watcher` lock, then acquires the lock only briefly to store the result.
- `McpServer` gains `file_watcher_task` (JoinHandle of in-flight startup, aborted on shutdown) and `file_watch_stopped` (AtomicBool) so a late, off-lock store after shutdown is suppressed (watcher never resurrected post-shutdown).
- `stop_file_watching` sets `file_watch_stopped`, aborts the in-flight startup task, then briefly locks to `stop_watching()` (teardown still detached).

## Acceptance Criteria
- [x] `FileWatcher::stop_watching` and `EntityWatcher::drop` no longer block the caller on the notify-watcher teardown; the watcher object is dropped on a detached thread.
- [x] Measured: an in-process MCP server `shutdown()` drops from ~5s to milliseconds (record before/after for one server). -- DONE: `server.shutdown()` measured at **21us** (was ~5057ms). See profiling below. The lock-contention root cause is fixed.
- [ ] The previously-flaky in-process MCP-server cluster passes FAST under full-workspace load. -- NOT MET BY THIS FIX: a THIRD, distinct facet remains — FSEvents stream **REGISTRATION** (`.watch()`, ~4.5s) on the background startup task pins a tokio worker and blocks runtime/test teardown. Filed as follow-up 01KS68DBMDTERYQSEP04JPJR2Z. Per the approved "STOP and report if it still doesn't pass under load — do not re-add workarounds" instruction, this is reported, not forced.
- [x] No change to observable watch behavior: prompt hot-reload and entity-cache event emission still work (existing event tests pass).
- [x] `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean. (Verified for swissarmyhammer-tools + swissarmyhammer-entity; fmt --check clean workspace-wide.)
- [x] No serial guards or nextest `slow-timeout` overrides reintroduced.

## Tests
- [x] Unit/behavior test that `FileWatcher::stop_watching` returns promptly (`test_stop_watching_returns_promptly`).
- [x] Equivalent promptness guard for `EntityWatcher` drop (`entity_watcher_drop_returns_promptly`).
- [x] Off-lock-build guard `test_start_builds_without_shared_lock`: `FileWatcher::start` builds while the shared lock is held (proves it never needs the lock — would deadlock if it did).
- [x] Shutdown-promptness guard `test_stop_file_watching_returns_promptly_during_inflight_registration`: `stop_file_watching()` returns in <1s (measured **0.02s**) while a registration is in flight (handle registered + aborted, stopped-flag suppression).
- [x] Late-store suppression guard `test_stop_file_watching_suppresses_late_store`: a startup completing after shutdown does NOT resurrect an active watcher.
- [x] Existing watcher event tests still pass (no behavior regression).
- [ ] Full-workspace reliability: cluster passes fast under load. -- BLOCKED on the third facet (follow-up 01KS68DBMDTERYQSEP04JPJR2Z).

---

## IMPLEMENTATION FINDINGS (2026-05-21, claude-code) — STOPPED FOR GUIDANCE

The teardown detach AND the lock-contention fix (option 1 + non-blocking shutdown) are both DONE and verified. Together they fully fix in-process server `shutdown()`: **~5057ms -> 21us**.

### Profiling data (single `test_client_list_tools`, after BOTH fixes; probes since removed)
```
PROBE start_mcp_server   = 19.1 ms
PROBE create_test_client = 7.2  ms   (handshake)
PROBE list_tools         = 9.7  ms
PROBE client.cancel      = 0.5  ms
PROBE server.shutdown    = 21   us   <-- FIXED (was ~5057ms)
PROBE FileWatcher::start = 4.55 s    <-- THIRD facet (registration), not shutdown
PASS  test wall time     = 4.59 s
```

### Why the cluster is still slow (THIRD, distinct facet)
- All measured phases sum to ~36ms; the ~4.5s test tail is NOT in any awaited code.
- It is the synchronous FSEvents `.watch()` REGISTRATION (~4.5s) running on a background `tokio::spawn` startup task. `shutdown()` aborts that task, but `abort()` cannot interrupt a synchronous in-flight `.watch()`, so the tokio worker stays pinned until `.watch()` returns; the `#[tokio::test(multi_thread)]` runtime drop then waits for that worker.
- This is distinct from facet (1) teardown drop [fixed] and facet (2) shutdown lock-contention [fixed]. Fixing it requires running `.watch()` off a tokio worker (e.g. dedicated detached std::thread delivering the debouncer back via a channel) — an architectural change beyond the approved "fix shutdown now" scope. Filed as 01KS68DBMDTERYQSEP04JPJR2Z.

### Decision: STOP and report (per approved instruction)
The assigned fix is complete and verified with data. `shutdown()` is fixed (5s -> 21us). The remaining cluster slowness is a separate, newly-isolated facet (FSEvents registration), reported as a follow-up rather than forced with a workaround, exactly as instructed. Task left in `doing` pending guidance on whether to pursue the registration facet now.