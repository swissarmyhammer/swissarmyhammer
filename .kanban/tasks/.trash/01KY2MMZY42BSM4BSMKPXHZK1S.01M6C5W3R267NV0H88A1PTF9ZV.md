---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
title: 'flaky test: review_working_emits_progress_notifications_per_pair_when_token_supplied regresses progress monotonicity under concurrent nextest batch'
---
## What

`cargo nextest run -p swissarmyhammer-tools review` intermittently fails (roughly 1 in 3 runs observed) with a panic like:

```
progress regressed: ... 47 -> 46
```

in `crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs::review_working_emits_progress_notifications_per_pair_when_token_supplied`.

The test asserts `notifications/progress` values are monotonically non-decreasing across a `review working` run. It passes reliably when run standalone (`cargo nextest run -p swissarmyhammer-tools review_working_emits_progress_notifications_per_pair_when_token_supplied`) but fails intermittently as part of the full `review` batch, suggesting a process-wide concurrency/resource contention issue (e.g. shared embedder cache, `REVIEW_PIPELINE_GATE` semaphore, or CPU contention from other concurrently-running review pipeline tests in the same nextest binary) perturbs timing enough to violate an ordering assumption in the monotonicity check.

## Discovery

Found while adversarially double-checking task ^s41dsh4 (the review no-retry/INCOMPLETE-banner change). Confirmed NOT caused by that change: reproduced identically on a clean tree with ^s41dsh4's changes `git stash`ed (3 runs, 1 failure, same panic message).

## Repro

```
cd crates/swissarmyhammer-tools
for i in 1 2 3; do cargo nextest run -p swissarmyhammer-tools review 2>&1 | tail -8; done
```

Expect roughly 1 in 3 runs to fail with the monotonicity panic in `review_progress_notifications_test`.

## Acceptance Criteria

- [ ] Root cause identified: why concurrent execution with other `review` tests perturbs the progress-event ordering/counts enough to violate monotonicity.
- [ ] Fix applied (either in the test itself if it's asserting something not actually guaranteed under concurrency, or in the production progress-mapping code if there's a genuine race).
- [ ] `cargo nextest run -p swissarmyhammer-tools review` run 10x back-to-back with zero failures.