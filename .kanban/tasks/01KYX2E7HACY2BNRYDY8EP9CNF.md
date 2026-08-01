---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyynep7ddhmet7scvckzsefr
  text: |-
    Picked up. Research on the drain path, and the design decision.

    ## What the completion signal had to be

    There is no end-of-turn variant in the ACP `SessionUpdate` enum, and the notification
    stream never closes between turns (the broadcast lives as long as the agent/connection),
    so "wait for the sender to close" is not available per turn.

    The chain a pool collector drains is several ASYNCHRONOUS FIFO hops:

        backend broadcast -> trace_notifications hop -> handle.notification_rx
          -> drive::forward_notifications hop -> pool notifier -> per-turn collector

    Two consequences:

    - When the `session/prompt` response lands, every chunk has been SENT into the head
      channel but may still be in flight through the hops. That is exactly what the fixed
      500 ms sleep was covering up.
    - A barrier injected anywhere below the head can overtake chunks still sitting in an
      upstream channel, so "drain until my own receiver is empty" is NOT sound with more
      than one hop.

    Rejected alternatives, and why:

    - Longer sleep — forbidden by the card, and it only moves the race.
    - Quiet-period / idle drain — still wall clock, still races.
    - Drain until the collector's receiver is empty — unsound across hops (upstream residue).
    - Count the turn's notifications and drain until the count is reached — needs the same
      agent cooperation as a marker, and is more fragile.
    - Removing the hops so one channel remains — bigger redesign, and it silently breaks
      again the moment anyone adds a hop.

    ## Chosen design

    An in-band end-of-turn marker, emitted by the agent on the SAME channel as the chunks,
    as the last act of the turn. Every hop is FIFO, so the marker cannot overtake a chunk of
    its own turn: seeing it proves the reply is whole. This is robust to any number of hops.

    The marker lives in `agent-client-protocol-extras` (the crate both agents already depend
    on), alongside `MAX_TOKENS_META_KEY` / `PIN_ON_SAVE_META_KEY`, as
    `turn_complete_notification` / `is_turn_complete` / `TURN_COMPLETE_META_KEY` — an empty
    `SessionUpdate::SessionInfoUpdate` carrying `_meta.turn_complete = true`. An empty
    `SessionInfoUpdate` declares nothing, so a client that does not know the key ignores it.

    ## Latent second race found in the flaky test

    `notification_rx_is_the_pools_single_collected_stream` also had a SETUP race independent
    of the drain: it pre-loaded all six chunks into `notify_tx` and only then subscribed its
    collector. `forward_notifications` runs on the other worker thread and can copy a chunk
    into the notifier before that subscribe lands — and a tokio broadcast drops sends that
    have no receiver, so a LEADING chunk could vanish. The test now subscribes first, then
    feeds, which is also the order production runs in (the collector is spawned before the
    prompt is sent). No assertion was changed.
  timestamp: 2026-08-01T12:40:46.061337+00:00
- actor: claude-code
  id: 01kyynfc6n4kqr3tg427cktzh5
  text: |-
    Implementation landed. RED -> GREEN evidence and verification.

    ## RED (against the fixed-sleep drain)

    Unit level, `claude-agent`:

        thread '...a_chunk_delivered_after_the_old_fixed_window_is_still_collected' panicked
        assertion `left == right` failed: the drain must wait for the end-of-turn marker, not a fixed window
          left: "early "
         right: "early late"

    Drive level (drain temporarily reverted to the 500 ms sleep to re-prove it):

        thread '...a_chunk_forwarded_after_the_old_drain_window_is_still_collected' panicked
        assertion `left == right` failed: a chunk forwarded after the old 500 ms drain window must still be collected
          left:  "...\"suggestion\":\"extract a helper\",\"va"
          right: "...\"suggestion\":\"extract a helper\",\"validator\":\"ignored-by-agent\"}]\n```\n"

    The truncated left side is the corrupted JSON the verify/fleet parser reads — the
    production symptom. Both are GREEN after the fix.

    ## Changes

    - NEW `agent-client-protocol-extras::turn_complete` — `TURN_COMPLETE_META_KEY`,
      `turn_complete_notification`, `is_turn_complete`, with unit tests.
    - `claude_agent::spawn_notification_collector` ends its loop on the marker for its
      session, or on channel close. Both are real end-of-stream signals.
    - `claude_agent::collect_response_content` awaits the collector task instead of sleeping;
      returns `Result<String>`. `NOTIFICATION_COLLECTION_DELAY_MS` (500) is GONE, replaced by
      `NOTIFICATION_DRAIN_BACKSTOP_MS` (10 s) used ONLY as a hang guard: hitting it logs
      `tracing::error!` and returns `Err`, never a short string.
    - Emitters, all of them the last act of a turn on the chunks' own channel:
      `ClaudeAgent::prompt` (turn body split into `run_prompt_turn` so the marker fires on
      every exit, early return and error alike), `llama_agent::AcpServer::prompt`, the
      `acp-conformance` `MockAgentAdapter` (so every mock conforms), the validators
      `ScriptedAgent`, drive's `LateAnsweringAgent`, and `PlaybackAgent` replay (recorded
      fixtures predate the marker).
    - `pool::run_prompt` propagates the drain error (`PoolError::Agent`).
    - drive tests: subscribe-then-feed ordering, marker in band; the dual-feed negative
      control now closes its sources and awaits both forwarders before the marker, so the
      doubling assertion is deterministic. `ABANDON_IDLE_WINDOW_MS` keeps its 800 ms value
      but no longer derives from the deleted constant — the drain is no longer a silent
      window, because the marker is itself progress traffic.
    - ARCHITECTURE.md: the marker documented in the ACP section. No new dependency edges —
      both agents and acp-conformance already depend on `agent-client-protocol-extras`.

    ## Results

    - `notification_rx_is_the_pools_single_collected_stream`: 20/20 in isolation;
      3/3 full-suite runs of `rdeps(swissarmyhammer-validators)` (5021/5021 each).
    - `cargo nextest run -E 'rdeps(claude-agent)'` — 6089/6089 passed, including the
      real-local-model review E2E over ACP.
    - `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6391 tests; caught one
      stale assertion in `playback.rs` (it pinned "notification then response"), now updated
      to pin the marker between them; re-run green.
    - `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Side effect worth noting: the review pool no longer pays 500 ms per turn. The validators
    suite went from ~20 s to ~4 s.

    Follow-up filed as ^cv5b83m: `swissarmyhammer_agent::await_collector` has the same bug
    class (fixed 100 ms sleep) and additionally throws away ALL content on its timeout.
  timestamp: 2026-08-01T12:41:08.565304+00:00
- actor: claude-code
  id: 01kyyrcggasvve3ffb8hjqpxtb
  text: |-
    Adversarial self-review (/double-check) came back REVISE. All five findings are fixed.

    1. **Channel close was indistinguishable from the marker, so a truncated reply still
       returned `Ok`.** The collector broke identically on the marker and on `Closed`. Fixed:
       the collector task now returns `CollectorEnd::{TurnComplete, StreamClosed}`, and the
       drain errors on `StreamClosed` — a stream that closed before the agent said the turn
       was over is an INCOMPLETE drain, not a clean end. The test that codified
       partial-as-success (`a_closed_channel_ends_the_drain`) is now
       `a_channel_closed_before_the_marker_is_an_error`.

    2. **A lagged collector paid the full 10 s backstop and then hard-failed.** A lag means
       the broadcast dropped notifications that may have carried this turn's chunks OR its
       marker, so the reply cannot be proven whole either way. Fixed: the collector counts
       skipped notifications, and the drain fails immediately with a distinct "dropped N
       notifications" error as soon as the collector finishes — no backstop wait when the
       marker itself survived. Covered by `a_lagged_collector_is_an_error_not_a_reply_with_holes`
       (a two-slot ring forces a real drop). Before this card, a lag silently truncated.

    3. **ARCHITECTURE.md over-claimed.** It now names `swissarmyhammer-agent`'s
       `await_collector` as the one collector still draining on a fixed sleep (card ^cv5b83m).

    4. **Stale comment** in `pool.rs` still describing "the 500ms trailing drain" — updated.

    5. **acp-conformance topology.** The mock adapter emits the marker over the connection
       while a pool mock streams straight into the notifier, so ordering holds because the
       chunk send is awaited before `prompt` returns, not because of FIFO. Documented that
       constraint at the emit site: a mock that streams from a detached task would race it.

    While fixing (1) and (2) the 4-tuple return of `spawn_notification_collector` became a
    named `NotificationCollector` (task + collected text + counts + skipped), which also cut
    `collect_response_content` from four arguments to two.

    Re-verified after the revision:

    - `notification_rx_is_the_pools_single_collected_stream`: 20/20 isolated, and 3/3
      full-suite runs of `rdeps(swissarmyhammer-validators)` (5021/5021 each).
    - `cargo nextest run -E 'rdeps(claude-agent)'` — 6090/6090 passed.
    - `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6392/6392 passed
      (covers `acp-conformance` and the extras crate, which sit outside the other two sets).
    - `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

    A separate sweep for agents that could drive the drain without emitting the marker found
    none: the only `AgentPool` constructions are the production driver, the `with_pool` test
    seam (`ScriptedAgent`), and `pool.rs`'s own tests (`MockAgentAdapter` / `PlaybackAgent`);
    the production factory mints only `ClaudeAgent` and llama `AcpServer`. The two agent
    paths that reply without a marker (`ScriptedReply::Error`, playback's "no recorded call")
    both FAIL the prompt response, so `run_prompt` short-circuits before the drain.
  timestamp: 2026-08-01T13:32:00.394119+00:00
position_column: doing
position_ordinal: '8280'
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