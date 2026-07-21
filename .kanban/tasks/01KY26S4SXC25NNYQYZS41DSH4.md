---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky28fyfpc6jkfc43m48wpcvz
  text: 'Picked up. Research done: refusal path confirmed at review_op.rs (const ~886, check_review_completeness ~895, ReviewError::Incomplete ~198, call site ~558; refusal-pinning tests ~1640-1680). synthesize already stamps "results are INCOMPLETE" banner + carries tasks_failed/tasks_attempted. Also found a stale consumer: crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs accepts the "incomplete review:" refusal as a designed outcome (~lines 57-61, 190-209) — must be updated since the refusal is being deleted. Plan: RED test in review/tests.rs driving the registered tool with an all-erroring scripted agent expecting Ok+banner (fails today with Err(Incomplete)); then delete the refusal; rewrite review_op inline tests as report-contract pins; extend fleet test with single-attempt/progress pins.'
  timestamp: 2026-07-21T11:55:31.702859+00:00
- actor: claude-code
  id: 01ky28yserd6yctqrk0p7f5fk0
  text: |-
    Implementation landed via TDD.

    RED: new test `an_all_failed_review_returns_the_incomplete_flagged_report_not_an_error` in crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs drives the registered tool with a scripted agent that errors every validator fork; watched it fail with the exact refusal ("incomplete review: 15/15 fan-out tasks failed ... then retry"). Discovery: builtin validators also fan out (15 tasks, not 1), so the test pins `validators: ["deduplicate"]` for a deterministic 1/1 tally. Also extracted `plant_reviewable_change` from `planted_duplicate_fixture` so fixtures share the repo planting.

    GREEN: deleted `ReviewError::Incomplete`, `INCOMPLETE_REVIEW_FAILURE_RATE`, `check_review_completeness`, and the call site in `run_review_request_inner` — the tool returns the report unconditionally (synthesize's banner + counts carry the failure). Rewrote the four inline refusal tests as report-contract pins (all/majority/minority-failed flagged INCOMPLETE with tally; clean run unflagged) via shared `assert_flagged_incomplete`.

    Fleet regression guard: extended `one_failing_task_yields_zero_findings_without_aborting_the_rest` (validators fleet/tests.rs) — exactly one BAD_BODY prompt submission (no retry/resubmission), Planned totals = [2] (never re-planned), exactly one PairStarted/PairDone per (validator, file).

    Stale consumer fixed: crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs accepted the refusal as a designed outcome; its module doc and Err-branch now reflect "finished runs are never refused".

    Evidence: `cargo nextest run -p swissarmyhammer-tools review` 51/51 passed; `cargo nextest run -p swissarmyhammer-validators review` 161/161 passed (superset incl. fleet 39/39); `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean on tools/validators/agent; repo-wide grep for check_review_completeness / INCOMPLETE_REVIEW_FAILURE_RATE / ReviewError::Incomplete / "then retry" = 0 matches.
  timestamp: 2026-07-21T12:03:38.072082+00:00
- actor: claude-code
  id: 01ky295mc3cpakxe8ahrfxa53e
  text: 'Finish loop iteration 1: /implement landed green. Refusal path fully deleted (check_review_completeness, INCOMPLETE_REVIEW_FAILURE_RATE, ReviewError::Incomplete + call site); tool now always returns the report — INCOMPLETE banner + tasks_failed/attempted counts carry failure signal. New RED→GREEN e2e test an_all_failed_review_returns_the_incomplete_flagged_report_not_an_error (tests.rs) + single-attempt pins in fleet tests (one submission, Planned [2] unaffected, one PairStarted/PairDone per pair). Stale consumer fixed: review_real_model_e2e.rs no longer accepts the old "incomplete review:" refusal. cargo nextest: tools review 51/51, validators review 161/161; clippy -D warnings clean. Next: /test full gate → checkpoint commit → /review HEAD~1..HEAD.'
  timestamp: 2026-07-21T12:07:22.243632+00:00
position_column: doing
position_ordinal: '8280'
title: 'review: no retry on failed fleet units — return flagged INCOMPLETE reports instead of the retry-instructing refusal'
---
## What

**Context.** A ~9-hour production `review sha` run logged 14 fleet task failures, all "agentic loop exceeded the per-turn iteration cap (32 iterations)" (the `AGENTIC_LOOP_LIMITS` abort in `crates/llama-agent/src/acp/server.rs`, `max_iterations: 32` — local qwen validators driving whole-file review prompts). Research confirmed the engine already attempts each (validator, file) unit exactly once and degrades failures to zero findings (`handle_task_failure` in `crates/swissarmyhammer-validators/src/review/fleet.rs`); the growing wire progress total is the by-design lazy per-batch `Planned` announcement (`ReviewProgressState` doc in `review_op.rs`), not a re-queue. The one genuine retry semantic in the codebase is at the tool boundary: `check_review_completeness` (`crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`, ~line 895) converts a majority-failed run into `ReviewError::Incomplete`, whose error message instructs the caller to "Check the agent/backend health (and `review.concurrency`), then retry." — so the driving reviewer agent re-runs the entire review, including the units that will hit the same iteration cap again. That is the retry treadmill.

**Decision (user):** no retry. A unit that fails, fails this run; the report says so loudly; the finish loop's future review passes re-review the same scope and converge. Partial results are returned, never refused.

**Changes:**

- [x] In `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`: delete the refusal path — remove `check_review_completeness`, the `INCOMPLETE_REVIEW_FAILURE_RATE` const, and the `ReviewError::Incomplete` variant (and its call site in `run_review_request_inner`). The tool returns the `ReviewReport` unconditionally. This is safe against the original calcutron "all-failed run read as clean empty pass" symptom because `synthesize` already stamps `> ⚠️ {failed}/{attempted} review tasks failed — results are INCOMPLETE.` directly under the report header when any task failed (`crates/swissarmyhammer-validators/src/review/synthesize.rs`, ~line 203), and the structured `ReviewCountsView::failed()` / `attempted()` carry the tally to callers.
- [x] Rewrite the review_op unit tests that currently pin the refusal (`review_op.rs` tests around lines 1640–1680, the `check_review_completeness` assertions) to instead pin the new contract: a majority-failed (including 100%-failed) report is *returned*, its markdown contains the INCOMPLETE banner, and its counts expose the failure tally.
- [x] In `crates/swissarmyhammer-validators/src/review/fleet/tests.rs`: extend `one_failing_task_yields_zero_findings_without_aborting_the_rest` (~line 1958) to pin single-attempt semantics — the failing validator's prompt is submitted to the scripted agent exactly once (no fallback resubmission, no re-queue), and it emits exactly one `PairStarted`/`PairDone` per (validator, file) with the `Planned` total unaffected by the failure. This is the regression guard against ever introducing the per-unit retry we just decided not to have.

Out of scope (separate decision if wanted): raising the 32-iteration `max_iterations` cap for large local models.

## Acceptance Criteria

- [x] A review run where more than half (including all) of the fan-out tasks failed returns a `ReviewReport` — no tool error — whose markdown contains the `results are INCOMPLETE` banner and whose counts report the correct `tasks_failed`/`tasks_attempted`.
- [x] `ReviewError::Incomplete`, `INCOMPLETE_REVIEW_FAILURE_RATE`, and `check_review_completeness` are gone; `grep -rn "then retry" crates/swissarmyhammer-tools/src/mcp/tools/review/` returns nothing.
- [x] A fleet unit whose task fails is attempted exactly once: one prompt submission for that validator, one `PairStarted`/`PairDone` per pair, `Planned` totals unchanged by the failure.

## Tests

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` (inline `#[cfg(test)]`): replace the refusal assertions (~1640–1680) with tests asserting a majority-failed and an all-failed report are returned with the INCOMPLETE banner and populated failure counts.
- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests.rs`: extend `one_failing_task_yields_zero_findings_without_aborting_the_rest` to assert exactly one submission for the failing validator and exactly one `PairStarted`/`PairDone` per pair.
- [x] `cargo test -p swissarmyhammer-tools review` and `cargo test -p swissarmyhammer-validators fleet` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.