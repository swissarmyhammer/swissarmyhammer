---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb080
project: local-review
title: 'fix(review): wire incremental-tracking baseline recording into the agent-driven review path'
---
## What

The incremental `review working` tracking (task pnkrd77 / `.validators/.hashes/`) is **half-wired in production**: the subtract-FILTER runs but the baseline-RECORDER never fires, so `.validators/.hashes/` is never written and every review re-reviews the full set (`subtracted=0` forever). Zero duplicate-review avoidance.

### Evidence (live calcutron qwen run, 2026-06-15)
- 10 `review working` calls completed.
- Log shows the filter running: `review working: incremental tracking filter applied candidates=7 survivors=7 subtracted=0` (always 0).
- `../calcutron/.validators/.hashes/` directory **never created**, 0 entries.
- `../calcutron/.validators/.gitignore` is still the swissarmyhammer-directory default ("Keep validator definitions") — my `ensure_gitignore` (which adds the `.hashes/` ignore line) **never ran**.
- Zero baseline-record log activity.

### Root cause — recorder wired into only one of two parallel pipeline drivers
- `record_reviewed(...)` is called in exactly ONE place: `crates/swissarmyhammer-validators/src/review/synthesize.rs:335-344` (`run_review`, gated `is_working && tally.attempted > 0`).
- But the production tool path does NOT use `synthesize::run_review`. `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:254` (`run_review_request_inner`) calls `crates/swissarmyhammer-validators/src/review/drive.rs:94` `run_review_over_agent` → `run_pipeline_in_connection`, a **separate** agent-driven pipeline.
- Both drivers call `scope_review` (so the subtract-filter + its log fire in both — that's why the filter is visible), but only `synthesize::run_review` has the post-review recording block. `run_review_over_agent`/`run_pipeline_in_connection` was never given it.
- So the ACP/agent-driven path (used by the MCP review tool, the local-qwen backend, and `/finish`) completes reviews without ever recording a baseline.

This is a duplicate-but-divergent pipeline: two `run_review`-shaped drivers, the recorder added to only one. pnkrd77's tests passed because the `drive.rs` e2e test path that exercised recording went through the recording site; the live `review_op → run_review_over_agent` path was never asserted to write `.hashes/`.

## Fix
- Factor the post-review baseline-record step out of `synthesize::run_review` into a single shared helper (e.g. `record_baseline_if_working(scope, repo_path, loader, &work, &tally)` in `synthesize.rs` or `tracking.rs`) — do NOT copy the block into the agent path (no duplicate-but-different code).
- Call that one helper from BOTH `synthesize::run_review` AND the agent-driven tail (`run_pipeline_in_connection` / `run_review_over_agent` in `drive.rs`), after the report is synthesized, gated identically (`is_working && tally.attempted > 0`), best-effort (a tracking write failure is logged, never fails the review).
- Ensure the recorder path creates `.validators/.hashes/` and runs `ensure_gitignore` (it already does inside `upsert_entry`/`record_reviewed` — just make sure the agent path actually calls it).

## Acceptance Criteria
- [ ] After a real `review_op` `review working` over a repo with changes (the production/agent path, not just `synthesize::run_review`), `.validators/.hashes/<path>.yaml` entries exist for every reviewed file.
- [ ] `.validators/.gitignore` contains the `.hashes/` ignore line after that review (ensure_gitignore ran).
- [ ] A second `review working` with no further changes logs `subtracted>0` (and/or short-circuits to "Nothing in scope") instead of `subtracted=0`.
- [ ] The recording step exists in ONE shared helper called by both pipeline drivers — no duplicated record block.
- [ ] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` and `cargo clippy --all-targets -- -D warnings` green on touched crates.

## Tests
- [ ] A test that drives the **agent path** — `run_review_over_agent` (with the scripted agent harness) or `review_op::run_review_request` — over a seeded repo, then asserts `.validators/.hashes/*.yaml` entries were written and `.validators/.gitignore` has the `.hashes/` line. This is the gap pnkrd77 missed; it must fail before the fix and pass after.
- [ ] A two-pass test on the agent path: first `review working` records baselines; a second unchanged `review working` yields `subtracted>0` (files subtracted) — proving end-to-end duplicate avoidance.

## Workflow
- Use `/tdd` — write the failing agent-path recording test first (it reproduces the calcutron symptom), then factor the shared helper and wire it into `run_review_over_agent`.