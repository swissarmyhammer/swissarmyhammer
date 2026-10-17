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
- actor: claude-code
  id: 01m54x3mxsc8e0mph7g63trjye
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (567100625); 3 findings (3 confirmed, 1 refuted) — crates/swissarmyhammer-lsp/src/daemon.rs:1, crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:231, crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:1228
    - next: implement the 3 open findings in the "Review Findings (2026-10-17 07:13)" section, then review again.
  timestamp: 2026-10-17T12:24:10.425622+00:00
- actor: claude-code
  id: 01m54x4dhp14212s1f4q931y0f
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — daemon.rs, supervisor.rs, supervisor_slot.rs (new), server/code_context.rs, serve/mod.rs and others
    - test: green — nextest rdeps(lsp | tools), 5270 passed
    - commit: 567100625
    - review: findings — crates/swissarmyhammer-lsp/src/daemon.rs:1, crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:231, crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:1228
  timestamp: 2026-10-17T12:24:35.638257+00:00
- actor: claude-code
  id: 01m54y1v36ndexh9hv1rxe6rjn
  text: |-
    ### progress — review findings (2026-10-17 07:13)
    - Finding 1 (prompt cap): `daemon.rs` now holds the production code only (31 KB). The test module moved to `daemon/tests/` as a pure move: `mod.rs` (shared spec helpers, state, lifecycle, health check, shutdown; 48 KB), `jsonrpc.rs` (framing; 17 KB), `handshake.rs` (initialize handshake, stderr capture; 16 KB), `restart.rs` (backoff, failure limit, healthy uptime; 14 KB). `python_mock_spec` moved next to the other spec helpers in `mod.rs`. Check: a sorted-line diff of the old test section against the new files shows only the new module docs, the `mod` lines, and the `use super::*;` lines. 93 tests before, 93 after. Lines 1-775 of `daemon.rs` are byte-identical to HEAD. Public API not changed.
    - Finding 2 (race): `spawn_lsp_supervisor` takes the tenure `ShutdownFlag`. After `install_lsp_supervisor`, while it holds `start_guard`, the new `start_unless_stepped_down` reads the flag. When set, `remove_lsp_supervisor` (new, in `supervisor_slot.rs`) takes this supervisor out of the slot only if the slot holds it, and aborts its tasks; then `start_guard.shutdown()` runs and `start()` is not called. `shutdown_lsp_supervisor` cannot be used there: its `stop()` locks the supervisor, and the start task holds that lock (deadlock). `spawn_lsp_supervisor_if_leader` now takes `Option<ShutdownFlag>` (only a leader has one) instead of `is_leader: bool`.
    - Other start paths in the file: the TS indexing task, the watcher and the reconcile take the tenure flag; the LSP worker tasks are tied to the supervisor with `track_lsp_task`; the drain and the health loop read the flag. One gap that is not an LSP leak: `start_lsp_workers_if_leader` reads the flag from the workspace at worker start, not the tenure flag. Recorded as new task ^39423c6.
    - Finding 3 (reuse): `slot_holds` is now `#[cfg(test)] pub(crate) fn` at module level in `supervisor_slot.rs` (it uses the new `SlotState::holds`, which `track_lsp_task` and `remove_lsp_supervisor` also use). The copy in `server/code_context.rs` tests is deleted, and the tests import it.
    - TDD: `a_step_down_before_the_install_starts_no_lsp_server` failed first ("the supervisor of a stepped-down tenure must be shut down"), then passed. `remove_takes_only_the_given_supervisor_out_of_the_slot` passes.
  timestamp: 2026-10-17T12:40:39.782090+00:00
- actor: claude-code
  id: 01m54y244werb81t01m1by980a
  text: |-
    ### implement — changed
    - evidence: lsp: `crates/swissarmyhammer-lsp/src/daemon.rs`, new `crates/swissarmyhammer-lsp/src/daemon/tests/{mod,jsonrpc,handshake,restart}.rs`. tools: `crates/swissarmyhammer-tools/src/mcp/server/code_context.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/code_context/supervisor_slot.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`. New tests: `a_step_down_before_the_install_starts_no_lsp_server` (RED, then GREEN), `remove_takes_only_the_given_supervisor_out_of_the_slot`.
    - checks: `cargo fmt --check` clean; `cargo clippy -p swissarmyhammer-lsp -p swissarmyhammer-tools -p swissarmyhammer-cli --all-targets -- -D warnings` clean; `cargo nextest run --no-fail-fast -p swissarmyhammer-lsp -p swissarmyhammer-tools`: 1806 passed, 0 failed, 0 skipped, no warnings.
    - findings: 3 of 3 checked in "Review Findings (2026-10-17 07:13)".
    - next: /review
  timestamp: 2026-10-17T12:40:49.052188+00:00
- actor: claude-code
  id: 01m54yccx571ehfgz6dtymf1v7
  text: |-
    ### test — green (iteration 2)
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-lsp) | rdeps(swissarmyhammer-tools)'` ran 5272 tests. 5272 passed, 0 failed, 0 skipped. 54 tests were slow. 4 tests had a LEAK marker.
    - LEAK markers: code-context-cli::bin/code-context cli::tests::doctor_verbose; code-context-cli::bin/code-context cli::tests::global_json_flag; swissarmyhammer-tools::git_diff_integration_test test_multiple_files; swissarmyhammer-tools::git_diff_integration_test test_parser_registry_direct_entity_extraction.
    - `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, no warnings.
    - `cargo fmt --check`: exit 0, no differences.
    - The known flaky validators test did not fail.
    - next: review.
  timestamp: 2026-10-17T12:46:25.701068+00:00
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

## Review Findings (2026-10-17 07:13)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 12 file(s) reviewed, 5 not reviewed.

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-lsp/src/daemon.rs` — 265941 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

- [x] `crates/swissarmyhammer-lsp/src/daemon.rs:1` `review-engine/prompt-cap` — This file exceeds the review prompt cap — 265941 rendered bytes against the 262144-byte per-file cap — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:231` `completeness/invariant-propagation` — The step-down guard covers only a supervisor that is already in the slot. The start task builds the supervisor, then calls install_lsp_supervisor, and only after that starts the LSP servers. If a step-down runs between the build and the install, shutdown_lsp_supervisor finds an empty slot and does nothing. The install then puts the supervisor in the slot and start() launches the servers. The stepped-down process keeps live LSP children, which is the leak this change was meant to stop. The spawn_drain path sees the shutdown flag and skips the workers, but it does not stop the supervisor it already installed. After install_lsp_supervisor, check the tenure's shutdown flag while still holding start_guard. If the flag is set, shut the supervisor down and do not call start(). Alternatively, take the tenure's shutdown flag into the start task and make install fail when the flag is already set. Add a test that sets the flag between build and install and asserts that no LSP server starts.
- [x] `crates/swissarmyhammer-tools/src/mcp/server/code_context.rs:1228` `reuse/reuse` — The test helper `slot_holds` reimplements the check that `supervisor_slot.rs` already has. It is a private fn inside that file's `mod tests`, so this module cannot call it, and the same ptr_eq-on-`lsp_supervisor()` logic is written twice. If the slot check changes, one copy will drift from the other. Move `slot_holds` out of `supervisor_slot.rs` `mod tests` into a `#[cfg(test)] pub(crate) fn slot_holds` at module level in `supervisor_slot.rs`. Import it in `code_context.rs` tests and delete the copy at line 1227-1231.
