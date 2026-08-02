---
assignees:
- claude-code
position_column: todo
position_ordinal: e680
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