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
- actor: claude-code
  id: 01m550adwanmcvv5520x32g9kx
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (commit 8941e2cb3). 27 findings, 27 confirmed, 21 refuted. All 3 items of the prior section (2026-10-17 07:13) are checked. New section "Review Findings (2026-10-17 07:47)" is added. Files: crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:148, :209, :277, :311, :416; crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:33, :103, :113, :516, :607, :694, :782, :846, :908, :1025, :1084, :1103, :1109, :1204, :1266, :1283, :1316, :1322; crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs:250, :282, :306, :389.
    - next: Implement each open item in the 2026-10-17 07:47 section. Then review again. The task stays in review.
  timestamp: 2026-10-17T13:20:18.314367+00:00
- actor: claude-code
  id: 01m550bzd1tarbrn3jr5n20njg
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — daemon tests moved to daemon/tests/*, race fix, shared slot_holds
    - test: green — nextest rdeps(lsp | tools), 5272 passed
    - commit: 8941e2cb3
    - review: findings — 27 in crates/swissarmyhammer-lsp/src/daemon/tests/{handshake,mod,restart}.rs (13 reuse/reuse, 12 test-integrity/no-test-cheating, 2 code-hygiene/magic-numbers)
    - decision: all 27 are kept. No written rule drops findings on moved tests (the review skill only names the rule as an example). Part of the tests were written by this task in iteration 1, and the iteration 1 review could not read daemon.rs because of its size.
  timestamp: 2026-10-17T13:21:09.025414+00:00
- actor: claude-code
  id: 01m550jk8eysg3wmv4vj6eqdwy
  text: |-
    ### research — review findings (2026-10-17 07:47)
    - Scope is test code only: `crates/swissarmyhammer-lsp/src/daemon/tests/{mod,handshake,restart}.rs`. `jsonrpc.rs` uses none of the spec or spawn helpers. No production change is necessary.
    - `LspJsonRpcClient` owns the child stdin. When the client drops (shutdown, restart), the stdin pipe closes. Thus a mock can block on `sys.stdin.read()` and exit on EOF. This is a sync signal and replaces the `time.sleep(...)` linger in the handshake mocks.
    - tokio `Child::wait()` keeps the exit status, and `try_wait()` then returns it. Thus "wait for the child to exit" replaces the `sleep(200ms)` before a health check.
    - `initialize_handshake` returns `LspError::JsonRpc` when the write to stdin fails (EPIPE). A mock that exits before it reads the request (for example `true`) can give `JsonRpc` or `HandshakeFailed`, by timing. A mock that reads the request first and then exits gives `HandshakeFailed("unexpected EOF reading headers ...")` every time.
    - `sleep N` as a server: the write goes to the pipe buffer and the read blocks, so `start()` returns `LspError::Timeout(startup_timeout)` every time.
    - The watch channel keeps only the last value. A task that waits on `changed()` before `shutdown()` runs sees `ShuttingDown`, because `shutdown()` yields in `graceful_shutdown` while the state is `ShuttingDown` (with a child that does not exit).
    - The stderr drain task calls the injected `StderrFilter` with each line. A filter that sends each line to a channel lets a test check the drained lines.
    - tokio sleep never ends before its deadline, so `elapsed >= backoff_duration(n)` is an exact lower bound.
  timestamp: 2026-10-17T13:24:45.966740+00:00
- actor: claude-code
  id: 01m5513pbw5vywtadvsrsknpwq
  text: |-
    ### progress — review findings (2026-10-17 07:47)
    Test code only. No production change.
    - Shared helpers in `daemon/tests/mod.rs`: `spec_with` (one spec builder for `test_spec`, `cat_spec`, `silent_server_spec`, `mock_spec`), `MOCK_PRELUDE` + `mock_script` / `handshake_body` / `handshake_spec` / `serving_handshake_spec` / `exits_after_request_spec` (one Python handshake mock; the differences are parameters), `spawn_piped` / `spawn_echo` / `spawn_exiting` / `spawn_silent`, `daemon_for`, `daemon_in_tempdir`, `daemon_with_child`, `wait_for_child_exit`, `kill_and_reap`, `install_cat_client` (returns `InstalledClient`, which kills and reaps the helper on drop; the spawn uses `.expect`), `spawn_next_state_observer`, `assert_handshake_failed`.
    - Named constants: `TEST_HEALTH_CHECK_INTERVAL_SECS`, `MISSING_BINARY`, `IDLE_SERVER`, `UNUSED_WORKSPACE_ROOT`, `ECHO_PROGRAM`, `EXITING_PROGRAM`, `SILENT_PROGRAM`, `PYTHON_PROGRAM`, `LONG_SLEEP_SECS`, `SHORT_SHUTDOWN_GRACE`, `SIGNAL_TIMEOUT`, `INITIALIZE_REQUEST_ID`, `SERVE_UNTIL_STDIN_EOF`, `EOF_BEFORE_ANSWER_TEXT`, `RESTART_LIMIT_TEXT`, and the startup timeouts.
    - Integrity fixes:
      - Each mock reads the request before it fails, so the result is deterministic. Each handshake error test asserts `HandshakeFailed` and the exact message text. The two stderr tests assert the stderr text in the message. No `Timeout` arm stays.
      - The timeout test uses a server that never writes and asserts `Timeout(startup_timeout)`, the `Failed` reason and attempts 1.
      - The ShuttingDown test uses an observer task that waits on the next state change while `shutdown()` waits the grace on a `sleep` child. It asserts `ShuttingDown`.
      - The stderr filter test injects a filter that sends each line to a channel and asserts the lines from before and after the handshake.
      - The restart log test uses `traced_test` and asserts the info log with `attempt=3` and `delay_secs=4`.
      - The backoff tests assert `elapsed >= backoff_duration(n)` (not a lower literal).
      - The `..._increments_failure_on_bad_binary` test is renamed `test_restart_with_backoff_waits_then_reports_binary_not_found` and asserts that the count does not change. `test_restart_with_backoff_resets_on_binary_not_found` had the same cause; it is renamed `..._reports_not_found_for_a_missing_binary` and asserts the count.
      - `test_health_check_clears_client_on_exit` installs a client first, so its assertion can fail.
      - Sleeps that waited for a child exit are now `wait_for_child_exit`. The mock lingers are now `wait_for_stdin_eof()`.
    - Deleted: `test_state_transitions` (the finding allows it; the record_failure and state_rx tests cover the transitions) and `test_shutdown_clears_client` (its client was never set, so it could not fail; `test_shutdown_clears_client_with_installed_value` covers it).
    - Drain loop finding: neither test needs a drain. `rx.borrow()` already gives the last value. Both loops are removed, so no copy stays. The timeout-path test is renamed `test_shutdown_timeout_path_waits_the_grace_then_not_started`, because the ShuttingDown claim now has its own test.
    - Did not work: `sys.stdout.close()` in a Python mock does not close fd 1 (standard streams use `closefd=False`), so the daemon waited for the 5 s timeout. The prelude now has `close_stdout()`, which calls `os.close(1)`.
  timestamp: 2026-10-17T13:34:06.204668+00:00
- actor: claude-code
  id: 01m5513sewvt73qwh9vg025ac9
  text: |-
    ### implement — changed
    - evidence: 3 files — `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs` (44 KB), `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs` (9 KB), `crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs` (12 KB). Findings: 27 of 27 checked in "Review Findings (2026-10-17 07:47)" (13 reuse/reuse, 12 test-integrity/no-test-cheating, 2 code-hygiene/magic-numbers); 0 open on the card.
    - checks: `cargo fmt -p swissarmyhammer-lsp --check` clean; `cargo clippy -p swissarmyhammer-lsp --all-targets -- -D warnings` clean; `cargo nextest run -p swissarmyhammer-lsp --no-fail-fast`: 237 run, 237 passed, 0 skipped.
    - next: /review
  timestamp: 2026-10-17T13:34:09.372593+00:00
- actor: claude-code
  id: 01m5516r8m1sxnqbsw928wbdn9
  text: |-
    ### test — red (iteration 3)
    - evidence: `cargo nextest run --no-fail-fast -p swissarmyhammer-lsp` ran 237 tests. 237 passed, 0 failed, 0 skipped. 1 test has a LEAK marker. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, no warnings. `cargo fmt --check`: exit 0, no differences.
    - LEAK: `daemon::tests::handshake::test_initialize_handshake_full_happy_path_direct` (crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:210). This test is in the changed code. The test spawns a Python mock (`spawn_piped`) that serves until stdin EOF. The test does not wait for the child to exit and does not kill and reap it. Thus the child process and its pipes stay open after the test ends. Possible fix: end the test with the shared `kill_and_reap` or `wait_for_child_exit` helper.
    - slow: no test has a SLOW marker. The longest test is `daemon::tests::restart::test_restart_with_backoff_info_log_path_attempt_2` (4.015 s). Next are `test_health_check_all_with_failed_daemon_attempts_restart` (2.018 s) and `test_restart_with_backoff_waits_then_reports_binary_not_found` (2.014 s).
    - next: implement — remove the LEAK in handshake.rs, then test again.
  timestamp: 2026-10-17T13:35:46.452099+00:00
- actor: claude-code
  id: 01m551k5mwrsqae4er1yk7w27w
  text: |-
    ### progress — LEAK in daemon/tests (iteration 3)
    - Checked each test in `daemon/tests/*.rs` that spawns a child.
    - `handshake.rs`: 7 tests held a raw `Child` and did not reap it (`test_initialize_handshake_invalid_workspace_path`, `_stdin_unavailable`, `_stdout_unavailable`, `test_capture_stderr_returns_empty_when_no_stderr_handle`, `test_capture_stderr_reads_available_bytes`, `test_capture_stderr_returns_empty_on_timeout`, `test_initialize_handshake_full_happy_path_direct`). Each now ends with the new shared helper `kill_and_reap_child(&mut child)` (in `mod.rs`; `kill_and_reap(&mut daemon)` now calls it). `test_capture_stderr_reads_available_bytes` now uses `exec sleep`, so the kill stops the one process that holds the pipes (before, `sh` died and the `sleep` grandchild stayed alive).
    - Same cause in the daemon itself: on the shutdown timeout path the daemon dropped the child (SIGKILL by `kill_on_drop`, but no reap), and a second `start()` dropped the old child the same way. Tests could not reap these children. Fix in `daemon.rs`: new private `kill_and_reap(&self, &mut Child)`; `shutdown()` calls it after the grace timeout (`graceful_shutdown` now takes `&mut Child`); `start()` kills and reaps an old child before it spawns a new one; the two start failure paths use the same helper.
    - TDD: `test_shutdown_kills_long_running_child` and `test_start_after_previous_successful_start` now assert that the pid is not in the process table (`ps -o stat= -p <pid>`, new test helpers `child_pid` and `process_is_in_table`). Both failed first (the child stayed as a zombie), then passed.
    - restart.rs: each spawned child is already reaped (`wait_for_child_exit` or `shutdown`). jsonrpc.rs spawns no child.
    - LEAK markers are NOT at 0 in a parallel run, and the cause is not in the test code. Parallel run A: 1 leaky (`test_shutdown_resets_session_open_documents`). Parallel run B (same code): 6 leaky, 4 of them `jsonrpc` tests that spawn no process. `--test-threads=1` run: 0 leaky. Probable cause: a pipe fd inheritance race between test processes that nextest spawns at the same time on macOS. Recorded as new task ^ts2s5jt. A person must decide the fix (nextest config or upgrade).
  timestamp: 2026-10-17T13:42:33.372145+00:00
- actor: claude-code
  id: 01m551k9b19adfcetkk1v5kbee
  text: |-
    ### implement — stuck
    - evidence: 3 files — `crates/swissarmyhammer-lsp/src/daemon.rs`, `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs`, `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs`. `cargo fmt -p swissarmyhammer-lsp --check` clean; `cargo clippy -p swissarmyhammer-lsp --all-targets -- -D warnings` clean; `cargo nextest run -p swissarmyhammer-lsp --no-fail-fast`: 237 passed, 0 failed, 0 skipped, but 1 leaky (run A) and 6 leaky (run B, 4 in child-free `jsonrpc` tests); `--test-threads=1`: 237 passed, 0 leaky.
    - blocker: the requirement "0 LEAK markers" is not met in a parallel run. The remaining markers also hit tests that spawn no process, so the test code cannot remove them. New task ^ts2s5jt holds the evidence. A person must decide the fix.
    - next: a person decides on ^ts2s5jt; the child reap changes are ready for /review.
  timestamp: 2026-10-17T13:42:37.153945+00:00
- actor: claude-code
  id: 01m551ykrjrjjggwxdw5mmykrm
  text: |-
    step: test, iteration 4. Outcome: green.

    - `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-lsp)'`: 5270 tests ran. 5270 passed. 0 failed. 61 slow. 3 leaky.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean (exit 0).
    - `cargo fmt --check`: clean (exit 0).
    - `cargo nextest run -p swissarmyhammer-lsp --test-threads=1`: 237 tests ran. 237 passed. 0 LEAK markers.

    LEAK markers in the parallel run. All 3 tests are outside swissarmyhammer-lsp. No test starts a child process:
    1. code-context-cli `commands::ops::tests::parses_get_status`. This test only parses arguments. It starts no child process.
    2. swissarmyhammer-cli `commands::doctor::checks::tests::test_lsp_servers_check_empty_dir`. This test uses an empty directory with no project types. It starts no child process.
    3. swissarmyhammer-tools `git_tool_integration_test::test_mcp_dispatch_inline_diff`. This test makes an inline text diff. It starts no child process.

    These markers agree with the harness problem in task ^ts2s5jt. They do not come from the daemon change.
  timestamp: 2026-10-17T13:48:48.274541+00:00
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

## Review Findings (2026-10-17 07:47)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 8 file(s) reviewed, 4 not reviewed.

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:148` `test-integrity/no-test-cheating` — The assertion accepts either `HandshakeFailed` or `Timeout`. This weakens the test. Any error from a process that exits immediately passes, so the test does not prove which failure path ran. Assert on `HandshakeFailed` only, and check that the message contains the stderr text 'config not found'. This proves the stderr capture path that the test claims to cover.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:209` `reuse/reuse` — This test builds a piped `cat` child by hand. The same setup is written out again in three other tests in this file, so one shared spawn helper would replace all four copies. Add one helper, for example `fn spawn_piped_cat() -> Child`, and call it from all four tests. Keep the per-test `take()` calls, because they are the parts that differ.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:277` `reuse/reuse` — This test repeats the piped `cat` spawn block. The copy only differs from the others in the stdout `take()` that follows it. Call the shared piped-`cat` spawn helper here, then keep only the stdout `take()` and `drop` lines in this test.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:311` `reuse/reuse` — This test repeats the piped `cat` spawn block. The copy only differs from the others in the stderr `take()` that follows it. Call the shared piped-`cat` spawn helper here, then keep only the stderr `take()` line in this test.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/handshake.rs:416` `test-integrity/no-test-cheating` — The `LspError::Timeout(_)` arm passes with an empty body and no assertion. If the handshake times out, the test passes without proving the error path it names. The test cannot fail on that branch. Remove the timing-dependent Timeout arm. Make the child exit deterministically, for example by closing stdout before sleeping, and assert on HandshakeFailed only. If a timeout is a real outcome, assert that it carries a specific message, so the arm is not empty.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:33` `test-integrity/no-test-cheating` — `test_state_transitions` builds enum values and compares them with themselves. It exercises no daemon transition, so it cannot fail when the daemon's state logic breaks. It reads as coverage but proves nothing about transitions. Drive a real transition through `LspDaemon`, for example `record_failure` (Starting→Failed) or `start` on a missing binary (→NotFound), and assert the resulting state. Or delete the test, since `test_state_serialization` and the record_failure tests cover the same ground.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:103` `code-hygiene/magic-numbers` — The literal `60` for `health_check_interval_secs` is repeated in every spec helper and in several test specs. A reader cannot tell whether each copy is meant to match the others, and a change to the test default must be made at every site. Name it once, for example as a `const TEST_HEALTH_CHECK_INTERVAL_SECS`. Add one test-module constant for the health check interval and use it in each helper and in the inline spec at line 681.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:113` `code-hygiene/magic-numbers` — The fixture string `nonexistent-lsp-binary-abc123xyz`, which stands for a command guaranteed not to exist, is written out at many sites in both files. If one copy is edited, the BinaryNotFound tests can silently start to find a real binary. Define one `const MISSING_BINARY: &str = "nonexistent-lsp-binary-abc123xyz";` in the shared test module and use it at each site, so the sentinel is written once.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:516` `reuse/reuse` — test_graceful_stop_of_running_child builds a cat child inline, duplicating the spawn helper pattern in this file. Call the shared spawn helper.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:607` `test-integrity/no-test-cheating` — The test computes `saw_shutting_down` and then discards it with `let _ = saw_shutting_down;`. The test name says it checks the ShuttingDown transition, but no assertion depends on it. The test cannot fail if ShuttingDown is never emitted. Assert the transition directly. Use a watch receiver that records every state, or assert `saw_shutting_down` with `assert!`. If the watch channel coalesces states, test a path where ShuttingDown is observable, or rename the test to match what it proves.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:694` `test-integrity/no-test-cheating` — The test is named for the timeout path, but the assertion accepts any of four error variants (Timeout, HandshakeFailed, SpawnFailed, JsonRpc). It proves only that start() fails. The timeout path is never checked. Pick the timeout path deterministically, for example with a child that holds its pipes open and does not write. Then assert `LspError::Timeout`. If the variant is OS-dependent, split the test so each variant has its own case.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:782` `test-integrity/no-test-cheating` — The test is named `..._increments_failure_on_bad_binary`, but it never asserts that `consecutive_failures` increased. It checks the error, the elapsed backoff, and the NotFound state. The named behavior is unchecked. Assert the counter after the call. Or rename the test to match what it checks, such as `test_restart_with_backoff_waits_then_reports_binary_not_found`.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:846` `test-integrity/no-test-cheating` — The test is named for the stderr filter task, but it never checks stderr. It asserts only a successful start, a Running state, and a clean shutdown. The stderr drain path could break and this test would still pass. Assert the stderr outcome the test claims to cover, for example through the stderr filter or capture hook. Otherwise rename the test to match what it checks, such as `test_start_with_stderr_output_reaches_running`.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:908` `reuse/reuse` — test_start_success_creates_shared_client builds its own copy of the handshake mock script. It differs from the other copies only by a sleep, so the script is reimplemented instead of shared. Call the shared handshake script constant, passing the sleep duration or any other difference as a parameter.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1025` `reuse/reuse` — test_start_after_previous_successful_start repeats the handshake mock script instead of reusing a shared one. Its only difference is a longer sleep. Use the shared handshake script constant and pass the longer sleep as a parameter.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1084` `reuse/reuse` — test_health_check_exited_process_full_contract builds a true child inline with the same settings as the other child spawns in this file. Use the shared spawn helper.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1103` `test-integrity/no-test-cheating` — The helper spawn is wrapped in `if let Ok(mut helper) = helper_child`. If the spawn fails, every contract assertion in `test_health_check_exited_process_full_contract` is skipped and the test passes. Replace the `if let Ok` with `.expect("spawn cat helper")` and remove the conditional.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1109` `reuse/reuse` — test_health_check_exited_process_full_contract repeats the cat-backed client install that appears in other tests in this file. Call the shared install helper instead of the inline spawn and install.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1204` `reuse/reuse` — The kill-then-wait cleanup on an optional child is written out again. The same cleanup appears earlier in this file. Add a helper, for example kill_and_reap(&mut daemon), and call it from both tests.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1266` `reuse/reuse` — test_shutdown_timeout_path_emits_shutting_down_then_not_started builds a sleep child inline with the same settings as the other spawn sites. Use the shared spawn helper.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1283` `reuse/reuse` — The loop that drains pending watch changes is written out again. The same loop already appears in another test in this file, so it is reimplemented instead of shared. Add a helper, for example drain_state_changes(&mut rx), and call it from both tests.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1316` `test-integrity/no-test-cheating` — The client-install assertions in `test_shutdown_clears_client_with_installed_value` run only inside `if let Ok(mut helper)`. If the spawn fails, the test passes without checking that shutdown clears the client. Use `.expect("spawn cat helper")` so a failed spawn fails the test.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/mod.rs:1322` `reuse/reuse` — test_shutdown_clears_client_with_installed_value repeats the cat-backed client install from other tests in this file. Call the shared install helper.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs:250` `reuse/reuse` — test_health_check_multiple_calls_after_exit builds a true child inline with the same settings as other spawn sites. Use the shared spawn helper.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs:282` `test-integrity/no-test-cheating` — The test is named `..._info_log_path_attempt_2`, and its doc comment says it exercises the sleep and restart info-log path. It asserts only that at least 3 seconds elapsed and that the state is NotFound. No log output is checked, so the logging path is not proven. Check the info log with `#[tracing_test::traced_test]` and `logs_assert`, as the nearby limit-log test does. Or rename the test and drop the log claim from its doc comment.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs:306` `reuse/reuse` — test_restart_with_backoff_success_path copies the handshake mock script that already appears in mod.rs. The copy lives in another module, so it drifts from the others. Import the shared handshake script constant from the parent tests module and use it here.
- [x] `crates/swissarmyhammer-lsp/src/daemon/tests/restart.rs:389` `reuse/reuse` — test_health_check_failed_attempts_tracks_consecutive_failures builds a true child inline with the same settings as other spawn sites. Use the shared spawn helper.
