---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8f80
project: local-review
title: A saturated/failed review must not render as a clean empty pass
---
## Problem

When fan-out tasks fail (e.g. the shared llama queue rejects them — see the queue-backpressure task), each failure is degraded to zero findings with only a `WARN`, and the pipeline still completes "successfully" and renders an **empty** `## Review Findings` section — byte-identical to a genuinely clean diff. A reviewer (human or `/finish` loop) cannot tell "nothing wrong" from "the review didn't actually run."

Observed on calcutron: 60/60 fan-out tasks failed with `Queue is full`, yet the tool returned a clean empty report and `counts` of all zeros.

## Where it happens

- `swissarmyhammer-validators/src/review/fleet.rs` `collect_task` — every failure (`Ok(Err)`, `Err(recv)`, parse failure) returns `Vec::new()`; the failure count is logged but **not propagated** to the report.
- `swissarmyhammer-validators/src/review/verify.rs` — same `refuting by default` on failure.
- `swissarmyhammer-validators/src/review/synthesize.rs` — `ReviewReport` / counts carry no notion of attempted-vs-failed tasks.
- `swissarmyhammer-tools/.../review/review_op.rs` `ReviewCountsView` — surfaces only blockers/warnings/nits/confirmed/refuted; no failed/dropped count.

## Fix direction

- Thread an attempted/failed/dropped tally through `run_fleet` → `run_review` → `ReviewReport`/`ReviewCountsView`.
- Render a failure summary in the report markdown (e.g. "⚠️ 60/60 review tasks failed — results are INCOMPLETE").
- When the failure rate exceeds a threshold, the op should return an **error** (incomplete review) rather than an empty success, so callers don't treat a wedged run as a pass.

## Acceptance criteria

- `ReviewReport`/`ReviewCountsView` expose attempted + failed task counts.
- The rendered report visibly flags an incomplete run.
- A review where a majority of fan-out tasks failed surfaces as a tool error, not an empty clean report.
- Tests in `fleet.rs`/`synthesize.rs` cover the all-failed and partial-failure cases.