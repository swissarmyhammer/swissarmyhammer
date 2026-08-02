---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyze2twd0232ew1047zwgcek
  text: |-
    Picked up as the explicit follow-up to ^8ep9cnf. Read that card's full history (7 review rounds) before starting — it fixed the SAME bug class in `claude_agent::collect_response_content`, on the OTHER collector behind `claude-agent`/`llama-agent`. This card is `swissarmyhammer_agent::await_collector`, the private collector behind `swissarmyhammer_agent::execute_prompt` (the CLI/kanban AI-panel path).

    ## Blast radius, checked before changing anything

    - `await_collector`, `spawn_collector_task`, `notification_collector`, and the test-only `spawn_notification_collector` are all private/`#[cfg(test)]` to `crates/swissarmyhammer-agent/src/lib.rs`. Grepped the whole workspace: their only callers are inside that same file (`drive_prompt_turn` and the test module). Changing their signatures has zero external blast radius.
    - `execute_prompt` (the public entry point) keeps its signature unchanged — the fix is entirely inside `drive_prompt_turn`'s internals.
    - Dispatched a fork to check whether any ACP agent driven through `execute_prompt` could skip the end-of-turn marker. Finding: `AcpAgentHandle` is constructed in exactly two places (`wrap_claude_into_handle`, `wrap_llama_into_handle`), both wrapping `claude_agent::ClaudeAgent` / `llama_agent::AcpServer` — both of which already emit the marker unconditionally as the last act of `prompt`, post-^8ep9cnf. `NoopAgent` (the post-consume placeholder) errors immediately on `connect_to` and can never reach a live turn. No mock/test agent in this crate needed a marker-emission fix — unlike ^8ep9cnf, which had to update `ScriptedAgent`, `LateAnsweringAgent`, and `PlaybackAgent` in other crates. The marker reaches this collector for free: it rides the same broadcast sender/receiver pair the agent already publishes on (via `trace_notifications`, which forwards every notification, including the marker, unmodified and in FIFO order).

    ## RED (against the fixed-sleep drain)

    Added a temp test using the CURRENT (unfixed) API — a chunk sent 250ms after collector start, well past the old 100ms sleep:

        thread 'tests::temp_red_check_late_chunk_is_dropped_by_fixed_sleep' panicked
        assertion `left == right` failed: a chunk delivered after the old fixed drain window must still be collected
          left: "early "
         right: "early late"

    Confirmed RED, then implemented the fix and replaced the temp test with the final marker-based version (see below) — GREEN.

    ## Fix

    Reused the ^8ep9cnf design (`claude_agent::CollectorEnd` / `DrainReport` pattern), adapted locally to this crate's shape:

    - `notification_collector` now loops on `notification_rx.recv()` and breaks on `CollectorEnd::TurnComplete` (session's `is_turn_complete(&notification)` marker) or `CollectorEnd::StreamClosed` (channel closed) — never on a timer or `cancel_token`.
    - Removed the `cancel_token: CancellationToken` plumbing entirely from `spawn_collector_task`/`notification_collector`/`spawn_notification_collector`/`await_collector`/`drive_prompt_turn`/`run_prompt_connection` — it existed only to force-stop the old sleep-based collector and has no role once the drain ends on a real signal. `tokio_util::sync::CancellationToken` import removed.
    - `NOTIFICATION_COLLECTION_DELAY_MS` (100ms sleep) deleted. New `NOTIFICATION_DRAIN_BACKSTOP_MS = 10_000` — a hang guard ONLY, mirroring `claude_agent::NOTIFICATION_DRAIN_BACKSTOP_MS`.
    - `await_collector` now returns `AcpResult<(String, u64)>` instead of `(String, u64)`. New `DrainFailure` enum (`CollectorDied`, `Backstop`, `Lagged`, `StreamClosed`) + `report_incomplete_drain` turn every incomplete-drain path into an `AcpError::PromptError` (logged at `error` with the identical message) — no path returns `String::new()` or a truncated string for an incomplete drain.
    - `drive_prompt_turn` always awaits the collector first (so the task is never left running past the turn), then propagates a prompt-level error ahead of a drain error (if the connection died, an incomplete drain is the expected consequence, not new information), then propagates the drain error if the prompt itself succeeded.
    - `build_agent_response`'s `messages_lost` parameter is unchanged (kept for its own independent unit tests); on every success path it is now always 0, since any lag turns the drain into an `Err` before reaching it.

    ## Tests

    Rewrote the 4 `spawn_notification_collector` unit tests to end via the marker or a closed channel (not `cancel_token.cancel()`), and added 5 new `await_collector` tests mirroring ^8ep9cnf's `collect_response_content_tests`:
    - `test_await_collector_collects_a_chunk_delivered_after_the_old_fixed_window` — the RED→GREEN regression test (700ms delayed chunk, `#[tokio::test]`, no `multi_thread` needed since the collector runs on `spawn_local` inside the same `LocalSet`).
    - `test_await_collector_channel_closed_before_marker_is_an_error`.
    - `test_await_collector_lagged_collector_is_an_error` (2-slot ring, 4 sends with no `.await` between them to force the drop).
    - `test_await_collector_missing_marker_hits_the_backstop` (`#[tokio::test(start_paused = true)]` — added `test-util` to `swissarmyhammer-agent`'s dev-dependency `tokio` features in `Cargo.toml` to enable this).
    - `test_await_collector_success` adapted to the new `(CollectorEnd, String, u64)` / `AcpResult` shapes.

    ARCHITECTURE.md's end-of-turn-marker paragraph updated: it named this crate's `await_collector` as the one collector still on a fixed sleep; now states both collectors drain on the marker, and explains why `swissarmyhammer-agent` needed no emitter of its own.

    ## Results

    - `cargo nextest run -p swissarmyhammer-agent` — 97/97 passed (was 96 before; +1 net after removing the temp RED test and adding 5 new tests, net count includes the pre-existing `review_real_model_e2e` real-CLI test).
    - `cargo nextest run -E 'rdeps(swissarmyhammer-agent)'` — 1109/1109 passed, 0 skipped (includes `kanban-app`, `swissarmyhammer-cli`, `swissarmyhammer-tools`, real-model e2e tests).
    - `cargo fmt --all --check` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean.

    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-agent/src/lib.rs (await_collector/notification_collector/spawn_collector_task redesigned to drain on the ACP end-of-turn marker instead of a fixed 100ms sleep; incomplete drains now return `AcpError::PromptError` instead of `String::new()`), crates/swissarmyhammer-agent/Cargo.toml (added `test-util` to dev-dependency tokio features), ARCHITECTURE.md (end-of-turn-marker paragraph updated). RED confirmed against the pre-fix fixed-sleep drain (`left: "early ", right: "early late"`), GREEN after the fix. `cargo nextest run -p swissarmyhammer-agent` 97/97, `cargo nextest run -E 'rdeps(swissarmyhammer-agent)'` 1109/1109, fmt clean, clippy (workspace, all-targets, -D warnings) clean.
    - next: /review (the card stays in `doing`)
  timestamp: 2026-08-01T19:51:12.013398+00:00
- actor: claude-code
  id: 01kyze6jhs5qj5rpactphygk6m
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — notification_collector now ends on the ACP end-of-turn marker or channel close, never a fixed sleep; await_collector returns AcpResult<(String,u64)> with a DrainFailure enum (CollectorDied/Backstop/Lagged/StreamClosed) so no path returns an empty/truncated string; NOTIFICATION_COLLECTION_DELAY_MS (100ms) deleted, replaced by a 10s hang-guard backstop only; RED (late-chunk test failed against old code) -> GREEN confirmed
    - test: green — rdeps(swissarmyhammer-agent) 1109/1109, fmt clean, clippy clean
    - commit: f2bffc989
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-01T19:53:14.553925+00:00
- actor: claude-code
  id: 01kyzh2wg1kedr4q6xqr9z1r2f
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit f2bffc989) returned 26 candidate findings, all in `crates/swissarmyhammer-agent/src/lib.rs`. Blame-checked every reported file:line against `f2bffc989`:
      - 10 "error message capitalization" findings (lines 117/119/121/123/125/128/387/467/474/699/729/1051) — the real `AcpError` enum with those exact `#[error(...)]` messages lives at the enum definition, blame `ed9629afb3` (2025-12-31), untouched by this commit. The engine's cited line numbers don't even land on the enum (they land on unrelated context/doc-comment lines); the underlying code they describe predates this change either way.
      - 2 "hardcoded MCP server name" findings (lines 282/488) — real occurrences are at the `HttpTransport`/`HttpServerConfig` construction sites, blame `bd1c09b1cb` (2026-01-17) and `ed9629afb3` (2025-12-31). Pre-existing.
      - 1 "dispatch_claude_notification/dispatch_llama_notification duplication" finding (line 925) — real functions are pre-existing, blame `646b63b739`/`71441e9127` (2026-05-27/05-01). Pre-existing.
      - 13 "hardcoded test constant" findings (broadcast capacity 16, batch size 512, worker threads 2, various test timeouts) — all at line numbers below the first changed hunk (1437) or between changed hunks; none fall inside a diff hunk. Pre-existing.
      - Net result: zero findings attributable to this commit's diff (the `notification_collector`/`await_collector`/`DrainFailure`/backstop-constant changes, the `Cargo.toml` `test-util` feature, and the `ARCHITECTURE.md` update). The engine found nothing wrong in the actual changed code.
    - next: none — task moved to done
  timestamp: 2026-08-01T20:43:39.393671+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8c80
title: swissarmyhammer_agent::await_collector drains on a fixed 100ms sleep and drops all content on timeout
---
# Symptom

`swissarmyhammer_agent::await_collector` (`crates/swissarmyhammer-agent/src/lib.rs`) is the same
bug class as ^8ep9cnf, on the OTHER collector — the private one behind
`swissarmyhammer_agent::execute_prompt` (the CLI/kanban AI-panel path), not
`claude_agent::collect_response_content`.

It drains like this:

```rust
tokio::time::sleep(Duration::from_millis(NOTIFICATION_COLLECTION_DELAY_MS)).await; // 100
cancel_token.cancel();
match tokio::time::timeout(Duration::from_millis(500), collector_handle).await {
    Ok(Ok(result)) => result,
    Ok(Err(e)) => { warn!(...); (String::new(), 0) }
    Err(_)      => { warn!(...); (String::new(), 0) }
}
```

Two defects:

1. The drain window is a flat 100 ms of wall clock. Anything the forwarding hops
   have not delivered by then is cut off, so the reply comes back truncated
   under load.
2. On the timeout and task-error paths it returns `String::new()` — the whole
   reply is discarded and reported as a warning, so the caller sees an EMPTY
   response rather than an error.

# Changes

^8ep9cnf added the in-band end-of-turn marker
(`agent_client_protocol_extras::turn_complete_notification` /
`is_turn_complete`), which both the claude and llama agents now emit as the
last act of every turn. This collector can drain on it:

- End the collector loop when the marker for the turn's session arrives (or the
  channel closes), exactly as `claude_agent::spawn_notification_collector` does.
- Keep a generous timeout as a hang guard only, and report a hit as an error —
  never an empty or truncated string.

Do NOT fix this by lengthening the 100 ms sleep.

# Acceptance

- A test that delivers a chunk later than the old 100 ms window still collects
  it (RED against the current code).
- No path returns `String::new()` for a drain that did not reach the end of the
  stream.
- `cargo nextest run -E 'rdeps(swissarmyhammer-agent)'` passes; `cargo fmt --all`;
  `cargo clippy --workspace --all-targets -- -D warnings` clean. #review #test-failure