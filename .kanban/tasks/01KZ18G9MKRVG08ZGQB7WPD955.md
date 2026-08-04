---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5qd37ny1v4wd3agx7z288z
  text: |-
    ### implement — changed
    - Fixed the two cited stale comments:
      - `crates/swissarmyhammer-validators/src/review/fleet.rs`, on `render_file_block`: replaced "the scope stage rejects it with a hard error rather than trimming it to a slice" with a description of `batch_work_list` excluding the pair and reporting it as a `SkippedFile` gap.
      - `crates/swissarmyhammer-validators/src/review/scope.rs`, above the source-slice assembly in `compute_per_file_facts`: replaced "A file too large for the review `batch_size` is never trimmed here — `batch_work_list` rejects it" with skip-and-report wording matching `synthesize.rs`.
    - Swept the whole `crates/swissarmyhammer-validators/src/review/` directory for "hard error" / "hard-error" / stale reject wording. Found one more stale spot not cited in the card: `scope.rs`, on `FileWork::source_slice()` doc comment — "(see the field's invariants on wholeness and the `batch_size` hard error)" — fixed to reference `batch_work_list` excluding an oversized file as a `SkippedFile` gap.
    - Confirmed every remaining "hard error" mention in the directory (`synthesize.rs` x2, `scope.rs` x2 near `batch_work_list`/tests) already correctly says "never a hard error" — describing the current skip-and-report behavior, not stale.
    - Verification: `cargo nextest run -p swissarmyhammer-validators` → 366 tests run: 366 passed, 0 skipped. `cargo fmt --all` clean (no formatting diff on touched files). `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` → 3073 tests run: 3073 passed (1 slow), 0 skipped.
    - Doc-only change confirmed: `git diff` on the two touched files shows only comment text changed, no code lines. Zero behavior change.
    - next: /review
  timestamp: 2026-08-04T06:29:32.021867+00:00
position_column: doing
position_ordinal: '8380'
title: Two doc comments still claim batch_work_list hard-errors on an oversized file
---
Found while investigating ^k5wsxh0.

## Scope note — the budget half moved out

This card ORIGINALLY held two defects. The first — the batch budget measuring
raw `source_slice.len()` bytes while the prime renders numbered, blamed lines —
is now covered by ^6jsxjbc, together with the larger defect it is part of: the
budget (393,216) is about 4x the agent's `max_prompt_length` (100,000), so a full
batch is rejected with a bare `invalid_params` and the fleet tallies a failed
task.

Fixing the budget without fixing the cap mismatch would not make a fat batch
work, so the two belong in one change. ^6jsxjbc carries the short-line-file test
case this card asked for: a file the raw-byte budget lets through but the
rendered size does not.

This card keeps only the documentation cleanup.

## The stale comments

`batch_work_list` now returns `(Vec<WorkList>, Vec<SkippedFile>)` and reports an
oversized file as a skipped gap. It no longer hard-errors. Two comments still
describe the removed behavior:

- `crates/swissarmyhammer-validators/src/review/fleet.rs`, on `render_file_block`
- `crates/swissarmyhammer-validators/src/review/scope.rs`, above the source-slice assembly

`synthesize.rs` already documents the current behavior correctly, so these two
are the stale copies. Both date from the ^3rnvage change that introduced the
skip-and-report behavior.

## Acceptance

- Both comments describe the skip-and-report behavior the code actually has.
- No other comment in the review module still claims the hard-error behavior —
  sweep for it rather than fixing only the two cited sites.

#bug #review