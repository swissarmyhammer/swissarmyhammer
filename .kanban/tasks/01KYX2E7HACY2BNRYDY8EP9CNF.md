---
assignees:
- claude-code
position_column: todo
position_ordinal: d680
title: 'Flaky under full-suite load: collect_response_content drains notifications on a fixed 500ms sleep'
---
# Symptom

`swissarmyhammer-validators review::drive::tests::notification_rx_is_the_pools_single_collected_stream` failed ONCE in a full `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` run (5014 tests), then did not reproduce.

Evidence gathered while working card ^t6tw0kg:

- Isolation: passes 5/5.
- Under 36 CPU burners on 18 cores: passes 6/6. Plain CPU contention alone does NOT reproduce it.
- Pristine HEAD, full suite: 5013/5013 passed.
- With the ^t6tw0kg change, full suite re-run: 5014/5014 passed.

So it is a genuine latent non-determinism, not a break introduced by ^t6tw0kg (that card touched only `review/scope.rs` message construction and tests; this test exercises the ACP broadcast/forward/collect path and never calls scope resolution).

# Root cause

`claude_agent::collect_response_content` (`crates/claude-agent/src/lib.rs`) drains notifications by sleeping a FIXED window and then aborting the collector:

```rust
tokio::time::sleep(std::time::Duration::from_millis(NOTIFICATION_COLLECTION_DELAY_MS)).await;
collector.abort();
```

with `pub const NOTIFICATION_COLLECTION_DELAY_MS: u64 = 500;`.

Anything the spawned `forward_notifications` task and the collector have not delivered inside that fixed 500ms is silently dropped, so `collected_text` comes back TRUNCATED. The test then fails its `assert_eq!(collected_single, reply)` (or the `reply.len() * 2` half). The full suite includes GPU/llama model tests that can stall the runtime long enough to miss the window; a single test never does.

This is the same family as ^t681xdv, ^yh4m6ed and ^aekpq0b — load-dependent failures in the review notification path — but a DISTINCT root cause: those are about progress-tick ordering/monotonicity in the bridge, this is the fixed-sleep drain in the collector.

# Changes

Replace the fixed sleep with a deterministic completion signal. The collector already knows when the turn is done, so the drain should wait on that, not on wall-clock time. Options, best first:

1. Have the collector signal completion (a `oneshot`, or a `Notify` fired when the end-of-turn notification is seen) and `await` that, with a generous timeout retained ONLY as a hang guard that FAILS loudly rather than silently truncating.
2. If a bounded wait must remain, drain until the channel is empty AND the expected notification count is reached, rather than sleeping a flat 500ms.

Do NOT fix this by lengthening the sleep or by relaxing the test's assertions — a longer sleep makes every caller slower and still races, and the assertions are the contract.

Check the blast radius before changing the signature: `collect_response_content` is `pub` in `claude-agent`; find every caller with an inbound callgraph first.

# Acceptance

- `notification_rx_is_the_pools_single_collected_stream` cannot truncate: with the collector artificially delayed past the old 500ms window, the test still collects the full reply (add that as a regression test — it should FAIL against the current fixed-sleep code).
- No silent truncation path remains in `collect_response_content`: an incomplete drain must surface as an error or a failed assertion, never as a short string.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean. #review
#test-failure