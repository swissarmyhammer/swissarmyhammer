---
assignees:
- claude-code
position_column: todo
position_ordinal: d480
title: Review progress ticks must be monotonic under load (bridge sequencing, not a test relaxation)
---
# Problem

`crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs::review_working_emits_progress_notifications_per_pair_when_token_supplied` fails intermittently under full-suite load:

```
notifications/progress regressed between index 114 and 115 (MCP spec violation):
55 -> 53 (messages: Some("Reviewed src/payments.rs against test-integrity")
       -> Some("Reviewed src/live.rs against test-integrity"))
```

Seen once in `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` (5012 tests). The same test passes 3/3 in isolation and passed a second full run, so it is load dependent, not a code regression.

# Cause to confirm

The review pool fans out across workers. Each finished `(validator, file)` pair emits a progress tick carrying a pair count. When two workers finish close together, the count each one reads and the order the ticks reach the peer can disagree, so the client observes a count that goes backwards.

The MCP spec requires `progress` to increase monotonically, so the test assertion is correct. The fix belongs in the emitter: assign the tick sequence at ONE place (the progress bridge or the counter that feeds it) so a later-sent tick can never carry a smaller `progress` than an earlier-sent one. Do NOT relax the test.

# Acceptance

- The emitter assigns monotonic `progress` per call, proven by a test that emits from several concurrent workers and asserts the received sequence never decreases.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` passes, with this test run repeatedly (e.g. nextest retries or a loop) to show the flake is gone.

Found while implementing ^t6tw0kg (unrelated change: `list validators` rule bodies). #review