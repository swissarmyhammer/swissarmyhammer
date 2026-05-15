---
assignees: []
position_column: todo
position_ordinal: '9180'
title: 'Fix CLI error_scenarios tests: 5 timeouts in subprocess fallback under parallel load'
---
Five tests in `swissarmyhammer-cli/tests/integration/error_scenarios.rs` fail with "Test command timed out after 60 seconds" (exit 124) during `cargo nextest run --workspace`. When run in isolation the same `sah` subprocess completes in ~140 ms, so the root cause is subprocess oversubscription: the test harness routes every `tool kanban ...` call through the debug `sah` binary (see `ExecutionStrategy::Subprocess` in `swissarmyhammer-cli/tests/in_process_test_utils.rs:348-407`), which is wrapped in a 60-second timeout at line 294-309. During a full workspace run the machine is saturated and the wrapped call exceeds 60 s.

Failing tests (all in swissarmyhammer-cli/tests/integration/error_scenarios.rs):
- `test_commands_work_without_git`  (file:line 184)
- `test_error_message_consistency`  (file:line 293)
- `test_invalid_kanban_operations`  (file:line 51)
- `test_resource_exhaustion`        (around line ~90)
- `test_exit_code_consistency`      (124 s aggregate timeout; log shows several subprocesses succeeding before the wrapper wall-clock limit hits)

Error example: `assertion left == right failed: Kanban commands should succeed with auto-init, stderr: Test command timed out after 60 seconds; left: 124; right: 0`

What was tried: reproduced manually with `sah tool kanban task get --id nonexistent_id` in a fresh tempdir — exited 2 in 143 ms, so `sah` itself is fine. The failure only appears under the full parallel workspace run.

Proposed fix directions:
1. Run these tests in-process instead of going through `Subprocess` — extend `can_run_in_process` / `should_run_as_subprocess` to cover the `tool kanban ...` path.
2. Or add a `test-group` in `.config/nextest.toml` limiting `package(swissarmyhammer-cli) & test(error_scenarios::)` to low concurrency.
3. Or raise the 60 s wrapping timeout in `execute_via_subprocess` (line 297) for this test module — but that just hides the oversubscription.

Option 1 is the principled fix; option 2 is the pragmatic test-infra fix. #test-failure #tes