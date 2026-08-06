---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kza31t36jtrevt4w9gqhy64s
  text: |-
    Research done. Facts:

    - The packer `batch_work_list` (crates/swissarmyhammer-validators/src/review/scope.rs) makes the `SkippedFile` list: one entry for each (validator, file) pair whose rendered block is more than the budget.
    - `synthesize` (crates/swissarmyhammer-validators/src/review/synthesize.rs) groups the pairs by path with `group_skips_by_path` and writes only a warning block in the markdown. `ReviewCounts.skipped` counts the distinct paths. No finding is made, thus the gate stays clean.
    - The wire type `ReviewCountsView` (crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs) copies the counts for the tool JSON.

    Plan:
    1. `synthesize` adds one CONFIRMED `VerifiedFinding` for each skipped path into the verified stream, before the count/dedup/order steps. The finding says the file is more than the review prompt cap and tells the user to split it. All downstream consumers see a normal finding — no special handling.
    2. `ReviewCounts` gets `skipped_files: Vec<String>` (sorted distinct paths) and a getter. This removes the `Copy` derive; the one deref-copy site in review_op.rs changes to a clone.
    3. `ReviewCountsView` gets the same `skipped_files` field, so the tool JSON carries the file list.
    4. The markdown warning block stays unchanged.
    5. Tests to update: synthesize tests that assert a skip gives zero findings, and the tools tests at tests.rs (~lines 1556, 1598, 1640, 1867, 1921) that assert `counts.findings == 0` with `skipped == 1`. New RED tests first, per TDD.
  timestamp: 2026-08-05T23:10:02.854022+00:00
- actor: claude-code
  id: 01kza3ry0tm2aknaw3aeptcgzc
  text: |-
    Implementation complete, TDD RED→GREEN in two cycles.

    What changed:
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` — new `skip_findings` helper: each skipped path becomes one CONFIRMED `VerifiedFinding` (validator `review-engine`, rule `prompt-cap`, line 1) chained into the verified stream before count/dedup/order/render. `ReviewCounts` gains `skipped_files: Vec<String>` (distinct, sorted) with a getter; `Copy` derive removed. `group_skips_by_path` now computed once. The markdown warning block is unchanged, so existing text consumers keep working.
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` — `ReviewCountsView` gains `skipped_files: Vec<String>` on the wire JSON; `Copy` removed; `From<ReviewReport>` clones the counts and maps the list.
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — the five real-pipeline batch-size tests now assert `findings == 1` (or 2 in the mixed test), the `- [ ] \`src/lib.rs:1\`` checklist item, and `counts.skipped_files` on the tool JSON. These run the registered tool over a real git repo with a synthetic over-cap file through the scoping/batching path.
    - New engine tests: `a_skipped_file_becomes_a_confirmed_checklist_finding` (watched fail RED, then GREEN), `counts_carry_the_skipped_file_list_sorted_and_distinct` (RED as a missing-field compile error, then GREEN). Wire-shape test asserts `counts.skipped_files` serializes as `[]` when nothing is skipped.

    Verification: `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools` = 1895 passed, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all` applied.

    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-validators/src/review/synthesize.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs; 1895 tests green, clippy -D warnings clean
    - next: formal /review
  timestamp: 2026-08-05T23:22:40.538458+00:00
- actor: claude-code
  id: 01kza4y5nc1v559djx27w5m2cm
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (c45ba2d40) — 4 findings, 4 confirmed, 15 refuted, 0 skipped. Files: crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1289, crates/swissarmyhammer-validators/src/review/synthesize.rs:64, crates/swissarmyhammer-validators/src/review/synthesize.rs:785, crates/swissarmyhammer-validators/src/review/synthesize.rs:852
    - acceptance check: pass — the report lists no file as "not reviewed — would exceed the agent's prompt cap"
    - next: correct the 4 checklist items in "## Review Findings (2026-08-05 18:28)", then run /review again
  timestamp: 2026-08-05T23:43:00.780555+00:00
- actor: claude-code
  id: 01kza4ytbb107g1fa3qvsdc3yt
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 3 files (prompt-cap skip becomes a CONFIRMED review-engine finding; skipped_files in counts; real-pipeline tests)
    - test: green — cargo nextest --workspace, 13545/13545; fmt + clippy clean
    - commit: c45ba2d40
    - review: findings — 4 confirmed, 0 skipped; open: review_op.rs:1289 (3600 constant), synthesize.rs:64 (newtype params), synthesize.rs:785 (393_216 constant), synthesize.rs:852 (12 constant)
  timestamp: 2026-08-05T23:43:21.963218+00:00
- actor: claude-code
  id: 01kza5fvexcq7vhq87nnmv72mw
  text: |-
    All 4 review findings are corrected. Each cause is removed from the full file, not only from the cited line.

    1. review_op.rs — the magic number 3600 is now the named constant `TEST_KEEP_ALIVE_VERY_LONG` in the `concurrent_producers_still_emit_a_dense_monotonic_counter` test. The constant has a doc comment. It is the only unnamed long duration that was in the file.
    2. synthesize.rs — two public newtypes `TasksAttempted(pub usize)` and `TasksFailed(pub usize)` wrap the two counts. `FleetTally::new(attempted: TasksAttempted, failed: TasksFailed)` cannot get transposed arguments now. All callers changed: the `From<&FleetOutcome>` impl, `run_review`, the synthesize tests, the scope.rs tests, and the `report_with_tally` helper (and its 6 call sites) in review_op.rs. The newtypes are exported from `review/mod.rs`.
    3. + 4. synthesize.rs tests — all size/budget fixture literals are named constants with doc comments: `TEST_BATCH_BUDGET_BYTES` (393_216), `TEST_OVERSIZE_RENDERED_BYTES` (500_000), `TEST_OVERSIZE_ALT_RENDERED_BYTES` (400_000), `TEST_TINY_BUDGET_BYTES` (5), `TEST_TINY_OVERSIZE_BYTES` (10), `TEST_TINY_OVERSIZE_ALT_BYTES` (12). The `"500000"`/`"393216"` string assertions now derive from the constants. The one (1_000, 100) fixture now uses the tiny constants — its magnitudes were immaterial.

    Verification: `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` = 3112 passed, 0 failed, 0 skipped. `cargo fmt --all` applied. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    ### implement — changed
    - evidence: 4 files — crates/swissarmyhammer-validators/src/review/synthesize.rs, crates/swissarmyhammer-validators/src/review/scope.rs, crates/swissarmyhammer-validators/src/review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs; 4/4 findings checked; nextest 3112 green, clippy -D warnings clean
    - next: formal /review
  timestamp: 2026-08-05T23:52:40.157803+00:00
- actor: claude-code
  id: 01kza6kpg81xfawx8wkg05r9ga
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (27fadbd96). Engine counts: 8 findings, 8 confirmed, 34 refuted, 27 attempted, 0 failed, 0 skipped. All 8 findings ask for a refactor of test code that existed before this commit (review_op.rs:1379, 1383, 1384, 1506, 1673, 2018, 2033, 2079 — fixture literals in pre-existing tests). The review skill's written exception drops each of them. Effective new findings: 0. All 4 prior checklist items are checked. Acceptance check passed: skipped = 0, no file over the prompt cap.
    - next: none — task moved review → done.
  timestamp: 2026-08-06T00:12:14.728307+00:00
- actor: claude-code
  id: 01kza6m7af4gbkkhwwef80d69x
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 4 files, all 4 prior findings fixed (named constants, TasksAttempted/TasksFailed newtypes)
    - test: green — cargo nextest --workspace, 13545/13545; fmt + clippy clean
    - commit: 27fadbd96
    - review: clean — 0 effective findings (8 engine findings on pre-existing test lines dropped by the written rule); skipped 0
    - result: ^cdqt4v8 done
  timestamp: 2026-08-06T00:12:31.951622+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffae80
title: 'review engine: escalate "a file no validator could read" beyond a warning'
---
Design question recorded from ^t1y1c37, not built there.

# Problem

When one (validator, file) pair's rendered prompt exceeds the agent's prompt cap, the engine prints one warning line and reports the rest of the review as normal. Nothing fails. A file that stays over the cap is permanently outside that validator's coverage, and every review of it reads "clean" for that dimension. `crates/mirdan/src/install.rs` sat in that state until the split (^t1y1c37): 567352 rendered bytes against the 476042-byte budget, skipped by `duplication` on every run.

# Proposal to evaluate

Treat "a file no validator could read" as a coverage failure, not a warning:

- The `ReviewReport` carries the skipped pairs as structured data (`counts.skipped` exists; add the file list), so orchestrators can gate on it.
- The `/review` skill and the finish loop treat a skipped pair as a finding on the task: "file X exceeds the review prompt cap — split it", so the gate fails until the file shrinks.
- Optionally: the engine emits the skip as a CONFIRMED finding itself, so no consumer needs special handling.

# Acceptance

- A review whose scope contains an over-cap file cannot end `clean`; the skip is visible as a finding or a non-zero gate, not only as a warning line in markdown.
- A test proves the behavior with a synthetic over-cap file. #review #design

## Review Findings (2026-08-05 18:28)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1289` — Unexplained magic number 3600 hardcodes a timeout duration (one hour) for testing purposes. The specific value should be captured in a named constant to make the test configuration strategy explicit. Define `const TEST_KEEP_ALIVE_VERY_LONG: std::time::Duration = std::time::Duration::from_secs(3600);` (or use an existing long-timeout constant), then pass it directly: `TEST_KEEP_ALIVE_VERY_LONG,`.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:64` — Two parameters of the same primitive type with different meanings must use newtypes for compile-time safety and clarity. Define newtype wrappers around usize (e.g., TaskAttempted, TaskFailed) and update the signature: pub fn new(attempted: TaskAttempted, failed: TaskFailed) -> Self.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:785` — Unexplained numeric literal 393_216 in test fixture configures budget behavior — should be named constant. Extract as named constant (e.g., `const TEST_BATCH_BUDGET_BYTES: usize = 393_216;`) to clarify its role and centralize the value.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:852` — Unexplained numeric literal 12 in test fixture configures file size behavior — should be named constant. Extract as named constant (e.g., `const TEST_MEDIUM_SIZE: usize = 12;`) to document the test scenario variation.