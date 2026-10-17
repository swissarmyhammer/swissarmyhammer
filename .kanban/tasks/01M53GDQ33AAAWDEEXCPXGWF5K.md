---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m53gkrf8tt7c2ryb7mnf4x1y
  text: |-
    ### note — from the Swift port check
    - `shutdown()` must wait for a `start()` that is in progress. If it does not, a running start can add daemons after shutdown returns.
    - When the supervisor stops at the restart limit, log the error one time. Do not log a warning again at each health interval.
  timestamp: 2026-10-16T23:26:32.424187+00:00
- actor: claude-code
  id: 01m54v2ds72ky7tdjw4cyqv734
  text: |-
    ### research
    - `LspDaemon::start()` sets `consecutive_failures = 0` after each good `initialize`. Thus a server that crashes after `initialize` never reaches `MAX_CONSECUTIVE_FAILURES`.
    - At the limit, `restart_with_backoff` returns `Err` and `health_check_all` ignores it. No log. The daemon stays `Failed`.
    - `restart_with_backoff` sleeps the backoff while `health_check_all` holds the supervisor mutex. This is not changed by this task.
    - `spawn_lsp_supervisor` (tools `server/code_context.rs`) builds and starts the supervisor in a task, then calls `LSP_SUPERVISOR.set(..)`. The set fails after the first time, so the new supervisor drops.
    - `run_lsp_health_check_loop` runs forever. A new promotion spawns a second loop.
    - `start_diagnostics_watcher_with_notifier` returns a `JoinHandle`. The current code drops it, so nothing can stop the watcher.
    - The request server, the bus fan-out and the resource feed tasks also hold sessions and never stop.
    - `sah serve` exit: `apps/swissarmyhammer-cli/src/commands/serve/mod.rs` (`manage_http_server_lifecycle`, `finalize_stdio_server_shutdown`). Nothing calls `LspSupervisorManager::shutdown()`.
    - Race: if step-down comes while `start()` runs, the supervisor is not yet in the slot. Plan: put the supervisor in the slot before `start()`, hold its mutex during `start()`, and make `shutdown()` final (a `start()` after `shutdown()` spawns nothing).
  timestamp: 2026-10-17T11:48:33.191666+00:00
- actor: claude-code
  id: 01m54vbe5j3s0jr1amg6tzcem0
  text: |-
    ### progress — part 1 (lsp crate) done
    - `daemon.rs`: a good start no longer clears `consecutive_failures`. `health_check` clears it only after `MIN_HEALTHY_UPTIME` (5 min) of uptime since the last good start. `restart_with_backoff` logs the limit error one time (`restart_limit_reported`). `force_restart` and the uptime reset clear that flag. The backoff already existed; it now grows because the count is kept.
    - `supervisor.rs`: `shutdown()` is final (`shut_down` flag, `is_shut_down()`). A later `start()` or `health_check_all()` starts no server.
    - Tests (RED, then GREEN): `test_crash_after_each_good_initialize_reaches_the_failure_limit` (python fake server that exits after `initialize`), `test_restart_at_the_limit_logs_the_error_one_time`, `test_health_check_clears_failures_after_the_minimum_healthy_uptime`, `test_health_check_keeps_failures_before_the_minimum_healthy_uptime`, `test_start_after_shutdown_spawns_nothing`, `test_new_supervisor_is_not_shut_down`.
    - Next: replaceable supervisor slot in tools, shutdown on step-down, shutdown at `sah serve` exit.
  timestamp: 2026-10-17T11:53:28.498416+00:00
- actor: claude-code
  id: 01m54w4nxngnys53z15fx48zx9
  text: |-
    ### implement — changed
    - evidence: 13 source files. lsp: `daemon.rs`, `supervisor.rs`. tools: new `tools/code_context/supervisor_slot.rs` (replaceable slot with tracked task abort handles), `server/code_context.rs` (lock before install, start under lock, `LspStartup`/`LspWorkerSetup`, tracked request server/fan-out/resource feed/diagnostics watcher, health loop stops on the shutdown flag, `step_down_and_stop_lsp`), `tools/code_context/{mod,support,status}.rs`, `tools/diagnostics/mod.rs`, `diagnostics_resource.rs`, `tools/code_context/tests/tool.rs`, `mcp/mod.rs`. cli: `commands/serve/mod.rs` (`stop_lsp_servers()` on the HTTP and stdio exit paths).
    - tests: daemon failure-limit/log-once/healthy-uptime tests, supervisor final-shutdown tests, 6 slot tests (incl. shutdown waits for a start in progress), `step_down_then_promotion_leaves_a_live_supervisor_in_the_slot`. All seen RED, then GREEN.
    - checks: cargo fmt; clippy -D warnings (lsp, tools, cli) clean; nextest lsp+tools 1804/1804 passed; cli serve filter 49 passed.
    - note: the slot code and the server/code_context.rs code were written before their tests; RED was shown by disabling the new lines.
    - next: /review
  timestamp: 2026-10-17T12:07:15.637872+00:00
- actor: claude-code
  id: 01m54wfceecmsedtskrxy17ejw
  text: |-
    ### test — green
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-lsp) | rdeps(swissarmyhammer-tools)'` ran 5270 tests. 5270 passed, 0 failed, 0 skipped (55 slow). The output has no LEAK markers. `cargo clippy --workspace --all-targets -- -D warnings` is clean. `cargo fmt --check` is clean.
    - note: The two statusline tests `test_index_deleted_cwd` and `test_languages_deleted_cwd` are slow (about 224 s each). They pass.
    - next: review
  timestamp: 2026-10-17T12:13:06.382715+00:00
position_column: doing
position_ordinal: '8480'
title: Stop the LSP on leader step-down and limit crash restarts
---
## Problem
Many sourcekit-lsp processes can run for one workspace, and each new start does a full index again.
- `step_down()` (`crates/swissarmyhammer-code-context/src/workspace.rs`) sets only the worker shutdown flag. It does not stop the LSP supervisor. The health loop and the diagnostics watcher continue. The old LSP process continues next to the LSP of the new leader.
- When a process becomes leader again, `start_indexing_workers_after_promotion` (`crates/swissarmyhammer-tools/src/mcp/server/code_context.rs`) makes a new supervisor. `LSP_SUPERVISOR.set(...)` fails because it is a `OnceLock` (`support.rs`). The new supervisor is dropped, and `kill_on_drop` kills the new LSP after it starts to index.
- In `crates/swissarmyhammer-lsp/src/daemon.rs`, `consecutive_failures` goes back to 0 after each good `initialize`. A server that initializes and then crashes restarts with no limit, about every 60 s.
- Nothing calls `LspSupervisorManager::shutdown()` when sah exits.

## Fix
- On step-down, shut down the LSP supervisor and stop the diagnostics watcher.
- On promotion, use one supervisor slot that sah can replace (for example `Mutex<Option<...>>` or `ArcSwap`), not a `OnceLock` that fails silently.
- Reset `consecutive_failures` only after the server stays healthy for a minimum time (for example 5 minutes). Use a backoff between restarts. Stop at the limit and log an error.
- Call supervisor shutdown when `sah serve` exits.

## Acceptance criteria
- After a step-down, no LSP child process of the old leader stays alive.
- Promotion after step-down gives one working LSP, and that LSP is not killed.
- A server that crashes after each good `initialize` stops after the limit.

## Tests
- Unit tests in `daemon.rs` for the failure counter and backoff, with a fake server command that exits after `initialize`.
- A test for step-down, then promotion, that checks that the supervisor slot holds a live supervisor. #lsp-fix #bug