---
assignees:
- claude-code
position_column: todo
position_ordinal: ac80
title: 'Flaky under parallel load: review_working_emits_progress_notifications_per_pair_when_token_supplied'
---
## What

`review_working_emits_progress_notifications_per_pair_when_token_supplied` in `crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs` (the HTTP-transport progress e2e from ^jn2wjd5) fails intermittently when run as part of the full `cargo nextest run -p swissarmyhammer-tools` suite, but passes deterministically in isolation. Observed 2026-07-21: 1 fail in the full run at 11.86s, PASS on isolated rerun at 11.67s. Root cause is resource contention — the test boots a real in-process HTTP MCP server and drives a scripted review; under parallel load with the rest of the suite it can lag past a timing assumption (likely a notification-arrival wait or the server-startup/settle window).

Investigate and harden so it is deterministic under parallel load:
- Read the test's timing/wait logic. If it waits a fixed duration for notifications to arrive, replace with a bounded poll-until-condition (await the expected notification count with a generous ceiling) rather than a fixed sleep.
- Check whether it shares a process-global resource (the review pipeline gate `REVIEW_PIPELINE_GATE`, the shared embedder OnceCell, a fixed port) with sibling tests; if a fixed TCP port or shared server is the contention point, randomize/ephemeral-port it or serialize just this test.
- Consider the stdio e2e `review_progress_stdio_test.rs` as the reference: it is deterministic. If the HTTP e2e's value is fully covered by the stdio one plus unit tests, converting or marking it appropriately is acceptable — but prefer fixing the flake over deleting coverage.

Do NOT paper over it with a blanket retry or a `#[ignore]`. The bar is: green in the full parallel suite across repeated runs.

## Acceptance Criteria
- [ ] Root cause of the parallel-load flake identified and stated in the fix (timing wait vs shared resource vs port)
- [ ] The test passes deterministically inside the full `cargo nextest run -p swissarmyhammer-tools` run — not just in isolation
- [ ] No fixed `sleep`-based wait remains in the test's notification-arrival path (replaced by bounded poll-until-condition) OR the shared-resource contention is removed/serialized, whichever the root cause is
- [ ] Coverage is preserved (the per-pair progress assertion still holds); no `#[ignore]`, no blanket retry annotation

## Tests
- [ ] The test itself is the test: run `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` several times (e.g. 5x, or with `--test-threads` high) and confirm zero failures of this test across runs
- [ ] Run: `cargo nextest run -p swissarmyhammer-tools` (full package, parallel) green

## Workflow
- Reproduce first (run the full package a few times to catch a fail), then fix the identified root cause, then prove stability with repeated full-suite runs. #test-failure