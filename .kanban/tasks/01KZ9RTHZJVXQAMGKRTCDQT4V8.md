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
position_column: doing
position_ordinal: '8380'
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