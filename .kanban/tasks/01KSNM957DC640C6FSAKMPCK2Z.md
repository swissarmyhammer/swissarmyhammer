---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc080
title: 'Flaky: avp-common validator::runner::tests::test_execute_ruleset_runs_rules_in_parallel'
---
## DONE (2026-05-28)

Root cause: `test_execute_ruleset_runs_rules_in_parallel` asserted `elapsed.as_millis() < 500` for 3 rules × 200ms agent sleeps. Under heavy parallel CI load the wall clock stretched past 500ms even when execution was genuinely concurrent (scheduler contention), so the timing assertion flaked.

Fix: prove concurrency **structurally**, with no wall-clock threshold.
- Added a `BarrierAgent` mock whose `prompt` blocks on a `tokio::sync::Barrier(3)` until all 3 prompts are in flight together. The barrier can only release if the runner ran the rules concurrently.
- Rewrote the test to wrap `execute_ruleset` in `tokio::time::timeout(10s)` (well under the 30s per-rule budget). Parallel → all 3 reach the barrier → releases → completes in ms. Serial regression → first prompt blocks on the barrier forever → the 10s timeout fires → `expect()` fails with a clear message.
- `SlowAgent` is untouched (the timeout/throttle tests legitimately need real sleeps).

Verification:
- All 5 `execute_ruleset` tests pass (incl. the SlowAgent throttle test).
- Stress: 20/20 back-to-back runs of the rewritten test passed. The fix is deterministic — there is no longer any timing threshold that can flake under load.

Acceptance criteria:
- [x] Reproduced the failure mode (wall-clock-under-load) and removed the timing dependency.
- [x] Overlap asserted structurally (barrier), not by wall clock.
- [x] Passes repeatedly (20/20) alone; deterministic by construction.