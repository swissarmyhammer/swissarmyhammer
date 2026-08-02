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
- actor: claude-code
  id: 01kyz1r8k5ynx97qnrmk2p85hx
  text: |-
    Review findings fixed — all 12, plus one emitter defect the engine missed.

    ## The two findings that asked for a decision

    **Finding 5 — `matched_count` and the end-of-turn marker.** COUNT the marker; do not
    weaken the docstring. The marker is addressed to the collector's own session and the
    collector acts on it, so a counter that reports "notifications received for this
    collector's own session" must include it. The other option (narrowing the docstring to
    "only what `process_notification` saw") would have described an implementation detail
    instead of the stream. The counter also stopped being dead: it was destructured as
    `_matched_count` and never read, and it is now `matched_notifications` on every drain
    error report, where "how many of this session's notifications arrived" is exactly the
    context that explains a short reply.

    **Finding 9 — empty reply, `error!` versus `Ok("")`.** Lower the LOG to `warn!`; keep
    `Ok("")`. An empty reply is a fact about the turn, not a failure of the drain: the marker
    proved the stream ended exactly where the agent said it did. Three legitimate paths reach
    it — a turn cancelled before its first chunk, a turn that only made tool calls, and (after
    the 13th fix below) a prompt the agent rejected. Returning `Err` would conflate "the agent
    said nothing" with "chunks were lost", and every case where the reply CANNOT be proven
    whole already returns `Err` above this point: lag, a channel closed before the marker, the
    backstop, a dead collector task. So an empty `Ok` is unambiguous, and the caller can read
    it as "no text", never as "something went missing". `warn!` keeps it visible — an empty
    reply usually disappoints the caller — without claiming a failure the return value
    contradicts. Pinned by
    `collect_response_content_tests::a_turn_that_streams_no_text_is_an_empty_reply_not_an_error`.

    ## 13th requirement — the emitter gap

    `ClaudeAgent::prompt` had two `?` operators BEFORE the split that guarantees the marker:
    `validate_prompt_request` and `resolve_session`. A bad prompt on a valid, already-subscribed
    session returned without marking the turn complete, so that client's collector waited out
    the full 10 s `NOTIFICATION_DRAIN_BACKSTOP_MS` and then errored.

    Fix: resolve the session FIRST, then run the whole turn — request validation included —
    inside `run_prompt_turn`, whose every exit passes through `notify_turn_complete`. Session
    resolution is the right boundary: an unresolvable id names no notification stream, so
    there is nothing to mark; a resolved id may already have a collector on it, so everything
    after that point must mark the turn. Reordering does not weaken any contract — the
    `opaque_session_ids` tests that pin one uniform `invalid_params` not-found code across
    `prompt`/`cancel`/`set_session_mode` all send a well-formed prompt, and no test pins
    "validation error beats not-found".

    Regression test: `crates/claude-agent/tests/integration/turn_complete_marker.rs`.
    RED against the unfixed code:

        thread '...a_rejected_prompt_ends_the_turn_for_a_subscribed_collector' panicked at
        turn_complete_marker.rs:68:6:
        a rejected prompt must emit its end-of-turn marker, not leave the collector waiting
        for the backstop: Elapsed(())

    The 2 s bound is far below the 10 s backstop, so `Elapsed` IS the bug. GREEN after the
    fix. Finding 5's test was RED as "the collector must count 2 notifications for its
    session; it counted 1".

    ## The other ten

    1. `turn_complete.rs` — module docs gained two runnable examples (emit the marker;
       recognize it, with an ordinary chunk as the negative). Both run under
       `cargo test --doc -p agent-client-protocol-extras`.
    2. `playback.rs` — `SESSION_UPDATE_METHOD` const; all five occurrences in the file now use
       it, not only the two the finding cited.
    3. `lib.rs` re-export — doc comment naming what each of the three items is for.
    4. `NotificationCollector` — `#[derive(Debug)]`.
    6-8. One `DrainReport` struct gathers the shared context once (collected text, stop reason,
       total and matched notification counts, skipped) and `DrainReport::incomplete(message)`
       is the single report path. All FOUR sites go through it — the finding named three lines
       but said four, and the fourth is the `skipped > 0` lag branch. The `message` is now both
       the `error!` log line and the `AgentError::Internal` text, so the log and the returned
       error cannot drift apart. Existing assertions on "closed" / "backstop" / "dropped" still
       hold.
    10-12. `TEST_NOTIFICATION_RING = 64` with the rationale (a broadcast ring must exceed the
       notifications in flight before the collector task runs; 64 leaves an order of magnitude
       of margin so no test lags by accident) and its deliberate opposite
       `LAGGING_NOTIFICATION_RING = 2`, which forces the drop the lag test asserts on. The
       counter-polling constants got names too.

    ARCHITECTURE.md: one sentence on the marker's exit-path contract — a turn ends at the
    marker however it ends, and an unresolvable session id is the one case with none.

    ## Results

    - `cargo nextest run -E 'rdeps(claude-agent)'` — 6093/6093 passed (1 flaky:
      `kanban-app::ai_panel_e2e`, the known llama KV flake, green on retry).
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 5021/5021 passed.
    - `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6395/6395 passed (same
      ai_panel flake, green on retry).
    - `cargo test --doc -p agent-client-protocol-extras turn_complete` — 2/2.
    - `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings`
      clean.

    The review-coverage gap on `crates/llama-agent/src/acp/server.rs` stays OPEN: it is a
    tooling limit (the file exceeds the review batch size and `batch_size` is ignored), not a
    code finding, and nothing in this pass changes it.
  timestamp: 2026-08-01T16:15:42.693941+00:00
- actor: claude-code
  id: 01kyz3xazn0jjhsc342zdc6989
  text: |-
    Adversarial self-review (/double-check) came back REVISE with 7 items. All 7 are fixed,
    and two of them correct claims I made in the comment above.

    1. **The new integration test really spawned the claude CLI.** `AgentConfig::default()`
       has `spawn_claude_on_new_session: true`, so `new_session` exec'd the CLI and waited on
       its init line — 3.9 s of the test, a stray child process, and a dependency on the local
       install. Its own doc comment claimed the opposite. Now uses the headless seam
       (`spawn_claude_on_new_session: false`, as `mcp_http_session` does) and says so. The test
       runs in 0.219 s.

    2. **"Every exit emits the marker" was false three ways.** `resolve_session` is not only a
       not-found path: `resolve_session_with` maps an unreadable session store to
       `internal_error` (-32603), and THAT session may exist with a client draining it — a
       second no-marker case with a real stream. Panics and dropped futures also skip the emit.
       Fixed by making the emit unconditional on ONE exit path: `prompt` resolves, picks the
       marker's address, runs the turn, then emits. The address rule is explicit — a resolved
       session gets its canonical id, the same key the turn's chunks carry, and a resolution
       FAILURE gets the id the client literally sent, which is what that client's collector is
       waiting on. Panics and cancellation are now stated as the one way past the emit, which
       is exactly why the drain keeps a hang guard, instead of being papered over.

    3. **The empty-reply test did not test the change it was credited with.** It asserted
       `Ok("")`, which was already the behavior; only the log LEVEL moved. It now captures
       through a scoped subscriber (`swissarmyhammer_common::test_utils::CaptureWriter`, the
       pattern from `llama_agent::gpu_lock`) and asserts a WARN line with the message and NO
       ERROR line. Verified by reverting `warn!` to `error!`: the test FAILS. So finding 9's
       decision is genuinely pinned now, both halves of it.

    4. **Correcting my own claim about the emitter gap's in-repo impact.** No in-repo caller
       was stalling for 10 s. `pool::run_prompt` and `execute_prompt_with_agent` both `?` out
       of the prompt error BEFORE calling `collect_response_content`, so neither drains after a
       rejection. The in-repo gain is narrower: the orphaned detached collector task now ends
       on the marker instead of living until the broadcast closes. The full-stall scenario
       belongs to an external ACP client that drains regardless of the prompt result — real,
       but not something this repo was hitting today. The fix is still required protocol
       hygiene, and the regression test pins the contract ARCHITECTURE.md states.

    5. **llama-agent's ordering comment went stale.** `AcpServer::prompt_inner` validates
       content before resolving the session, under a comment claiming it mirrors claude-agent.
       The comment now explains why the two orderings legitimately differ: llama's `prompt`
       marks the turn complete for `request.session_id` whatever `prompt_inner` returns, so it
       has no ordering constraint; claude resolves first because it addresses the marker to the
       resolved id. Both errors are `invalid_params`, so only the message differs for a request
       that is both unknown-session AND unsupported-content. Behavior unchanged.

    6. **`DrainReport` had dropped two structured fields** — `error` (the join error) and
       `backstop_ms` survived only interpolated into the message, so a log query filtering on
       those names stopped matching. Fixed by giving the report a `DrainFailure` enum: one
       variant per failure path, and `incomplete` derives the message AND the per-failure
       fields from it. Every path still gains `matched_notifications` and `skipped`, and the
       call sites are now one line each.

    7. Missing blank line between two tests. Fixed.

    ## Re-verified after the revision

    - `cargo nextest run -E 'rdeps(claude-agent)'` — 6093/6093 passed, no flakes.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 5021/5021 passed.
    - `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6395/6395 passed.
    - `cargo test --doc -p agent-client-protocol-extras` — 5 passed, 1 ignored.
    - `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings`
      clean.
  timestamp: 2026-08-01T16:53:26.133252+00:00
- actor: claude-code
  id: 01kyz3xhe6vbynpjnbbd2q9wq1
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/agent-client-protocol-extras/src/turn_complete.rs, crates/agent-client-protocol-extras/src/playback.rs, crates/claude-agent/src/lib.rs, crates/claude-agent/src/agent_trait_impl.rs, crates/claude-agent/tests/integration/turn_complete_marker.rs (new), crates/claude-agent/tests/integration/mod.rs, crates/llama-agent/src/acp/server.rs (comment), ARCHITECTURE.md. All 12 review findings checked off plus the 13th (emitter gap). rdeps(claude-agent) 6093/6093, rdeps(swissarmyhammer-validators) 5021/5021, rdeps(agent-client-protocol-extras) 6395/6395, doctests 5 passed; fmt and clippy clean.
    - next: /review (the card stays in `doing`)
  timestamp: 2026-08-01T16:53:32.742022+00:00
- actor: claude-code
  id: 01kyz6ja4yex97attwm9yvnk07
  text: |
    ### review — findings
    - evidence: 2 findings — crates/agent-client-protocol-extras/src/playback.rs:290, crates/claude-agent/tests/integration/turn_complete_marker.rs:1. Scope d29a4ce25 (HEAD~1..HEAD). `review sha` hard-errored again on crates/llama-agent/src/acp/server.rs (319,069 bytes over the 262,144-byte batch budget), so the pass used per-file `review file` runs. 113 raw findings; 111 blame to 2025-12..2026-05 and are pre-existing. crates/llama-agent/src/acp/server.rs still un-reviewable (^3rnvage); its change here is comments only.
    - next: fix the two findings, then re-review. The end-of-turn marker invariant HOLDS by construction on all four exits of ClaudeAgent::prompt; the gap is that two of them have no test.
  timestamp: 2026-08-01T17:39:50.558593+00:00
- actor: claude-code
  id: 01kyz807ghqngb4dv53xfh6nbw
  text: |
    Review round 3 — both findings fixed.

    ## Finding 1 — one construction site for a `session/update` notification

    The engine swapped the two function names and its second range was stale. True
    locations, verified before editing: `playback.rs:288-292` was inside
    `send_turn_complete`, and `playback.rs:340-341` was inside
    `send_recorded_notifications`.

    New private helper `send_session_update_notification(tx, params)` holds the
    `Request::notification_v2(SESSION_UPDATE_METHOD.to_string(), Some(params))`
    construction and the `send_message` call. Both sites are now one line each.

    Fixed at the root, not only at the two cited sites: a sweep of the file for
    `session/update` shows the ONLY remaining `notification_v2` calls are the one
    inside the helper and a test's `session/cancel` (a different method). Nothing
    else in the file builds a `session/update` notification by hand.

    ## Finding 2 — one test per untested exit

    `ClaudeAgent::prompt` has three exits that return before any backend work. Only
    one had a test. Two added:

    - `an_unresolvable_session_id_ends_the_turn_for_a_subscribed_collector` —
      a ULID that names no session; `invalid_params`.
    - `an_unreadable_session_store_ends_the_turn_for_a_subscribed_collector` —
      `internal_error` (-32603).

    Each subscribes its collector on the id the CLIENT sends, never on a resolved
    id: that IS the addressing rule under test. Each asserts the drain finishes
    inside `DRAIN_MUST_FINISH_WITHIN` (2 s), so hitting the 10 s backstop cannot
    pass for success.

    **How the -32603 exit is staged.** `SessionManager` keeps its sessions behind a
    `std::sync::RwLock`, so a panic while a writer holds it poisons the lock and
    every later `get_session` fails — which `resolve_session_with` maps to -32603.
    The test induces exactly that through the manager's own public
    `update_session` with a panicking closure, on a real session it created. No
    test-only hook was added to production code. The default panic hook is silenced
    for the duration so a deliberate panic does not read as a crash in passing test
    output, then restored.

    **No claude CLI.** All three tests build the agent with
    `spawn_claude_on_new_session: false` — the headless seam `mcp_http_session`
    uses, factored into a shared `headless_agent()` helper. Each test runs in
    0.21 s, so nothing is exec'ing a CLI.

    ## RED evidence

    RED was produced by restoring the pre-fix shape in `prompt` — one `?` on
    `resolve_session` before the emit, which is exactly the code the
    `marker_session_id` fallback replaced:

        FAIL [   2.215s] an_unresolvable_session_id_ends_the_turn_for_a_subscribed_collector
        panicked at turn_complete_marker.rs:102:9:
        an unresolvable session id must emit its end-of-turn marker, not leave the
        collector waiting for the backstop: Elapsed(())

        FAIL [   2.218s] an_unreadable_session_store_ends_the_turn_for_a_subscribed_collector
        panicked at turn_complete_marker.rs:102:9:
        an unreadable session store must emit its end-of-turn marker, not leave the
        collector waiting for the backstop: Elapsed(())

        Summary [2.222s] 3 tests run: 1 passed, 2 failed

    `Elapsed` at 2.21 s IS the bug: the collector was on its way to the 10 s
    backstop. The third test (rejected prompt) still passed under the break, which
    is correct — it exercises the resolved path, which the `?` did not touch. After
    restoring the fix, `git diff` on `agent_trait_impl.rs` is empty, so the restore
    is byte-exact. All three GREEN at 0.21 s each.

    ## The module doc no longer overclaims

    The old first line, "Every `session/prompt` exit path emits the end-of-turn
    marker", was broader than the file proved and broader than the file CAN prove: a
    turn that reaches the backend needs the claude CLI, which these tests
    deliberately never spawn. The doc now says "every `session/prompt` exit that
    returns without running a turn", tabulates the three such exits with their error
    code and the id the marker is addressed to, explains why the first two are the
    reason the address is picked before resolution is known, and states that a
    backend-driven turn leaves through the same emit but is not driven here. The one
    way past the emit — a panic or a dropped future — is named, which is why the
    drain keeps its hang guard.

    ## Results

    - `cargo nextest run -E 'rdeps(claude-agent)'` — 6095/6095 passed, 2 skipped, no
      flakes (the real-local-model review E2E over ACP included, 139 s).
    - `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6397/6397
      passed, 2 skipped, no flakes.
    - `cargo test --doc -p agent-client-protocol-extras` — 5 passed, 1 ignored.
    - `cargo fmt --all` — no change to the edited files.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean.
  timestamp: 2026-08-01T18:04:55.185788+00:00
- actor: claude-code
  id: 01kyz80cykcw9619p9s1htx0gf
  text: |
    ### implement — changed
    - evidence: 2 files — crates/agent-client-protocol-extras/src/playback.rs, crates/claude-agent/tests/integration/turn_complete_marker.rs. Both round-3 findings checked. rdeps(claude-agent) 6095/6095, rdeps(agent-client-protocol-extras) 6397/6397, doctests 5 passed 1 ignored; fmt and clippy clean.
    - next: /review (the card stays in `doing`)
  timestamp: 2026-08-01T18:05:00.755032+00:00
- actor: claude-code
  id: 01kyz94n3x5s67zcg3m6phj67b
  text: |
    ### review — findings
    - evidence: 5 open findings — turn_complete_marker.rs:152, :191, :230 (engine, `worker_threads = 2`), plus turn_complete_marker.rs:17-21 and :13-15 (reviewer assessment on the module doc). Engine returned 7 findings / 7 confirmed / 2 refuted on `review sha HEAD~1..HEAD` (359a19014); 4 blame to 2025-12-24 and 2026-05-01 and are dropped as pre-existing.
    - notes: `review sha` did NOT error this round — this commit does not touch crates/llama-agent/src/acp/server.rs. The round-3 playback.rs duplication finding is closed; the engine raised nothing against the new `send_session_update_notification` helper. The `RwLock` poisoning that stages the -32603 exit is ACCEPTED: it drives the branch the production doc names, uses only the public API, and two asserts make a broken setup fail loudly. Half of the `worker_threads` remedy cannot compile — tokio's attribute macro needs a literal, so a named constant is impossible.
    - next: add the module comment that explains the 2 worker threads and record the macro limit; correct the module doc's addressing table and its count of three early exits.
  timestamp: 2026-08-01T18:24:48.765332+00:00
- actor: claude-code
  id: 01kyzbpc7gqscqbqb6p2g41qmq
  text: |
    ### implement — changed
    - The working tree had an interrupted prior session's changes already in place: `crates/claude-agent/tests/integration/turn_complete_marker.rs` had its module doc rewritten (dropped the "addressed to" table column, added a `# Which id the marker is addressed to` section, added a `# What these tests do NOT pin` section, added a `# Why worker_threads = 2` section with the macro-limitation note) and gained a fourth test, `a_pre_cancelled_session_ends_the_turn_for_a_subscribed_collector`, covering the pre-cancelled `Ok(StopReason::Cancelled)` exit. But `crates/claude-agent/src/agent_trait_impl.rs` still held a deliberate RED-check break: `let _ = &marker_session_id; // TEMP RED CHECK: emit removed` in place of `self.notify_turn_complete(&marker_session_id).await;`.
    - Restored the `notify_turn_complete` call. `git diff` against HEAD on that file is now empty (byte-exact restore).
    - Verified the restored doc/test changes fully resolve all 3 open round-4 findings: worker_threads=2 magic-number finding (module comment + macro-limitation note added, no attempt to re-introduce the impossible named constant), the addressing-table finding (table restructured to drop the misleading distinction, addressing rule explained separately with the "only the unresolvable-id test can tell them apart" statement), and the exit-count finding (doc now states the exact reading — "every exit reachable before `run_prompt_turn` calls `send_user_message_chunks`" — names it complete under that reading, and a `# What these tests do NOT pin` section lists the four remaining unpinned exits by name: `prepare_session_for_turn`, `check_turn_limits`, `get_updated_session`, and the turn-limit `Ok` exit). Cross-checked the doc's claims against `agent_trait_impl.rs::prompt`/`run_prompt_turn` line by line — accurate.
    - Did not touch `crates/llama-agent/src/acp/server.rs` (^3rnvage, out of scope).
    - evidence: `cargo nextest run -p claude-agent turn_complete_marker` — 4/4 passed (including the new pre-cancelled test). `cargo fmt --all` — no changes to source. `cargo clippy --workspace --all-targets -- -D warnings` — clean. `cargo nextest run -E 'rdeps(claude-agent)'` — 6096/6096 passed, 2 skipped. `cargo nextest run -E 'rdeps(agent-client-protocol-extras)'` — 6398/6398 passed, 2 skipped. All 5 open round-4 checklist items marked `[x]` on the task description with resolution notes.
    - files touched: crates/claude-agent/src/agent_trait_impl.rs (1-line restore only).
    - next: /review
  timestamp: 2026-08-01T19:09:26.640212+00:00
- actor: claude-code
  id: 01kyzbrysszexb4x0w20c5zta6
  text: |-
    ### finish iteration 5 — changed
    - implement: changed — restored 1-line RED-check regression in agent_trait_impl.rs (already matched HEAD after restore); round-4 doc/test fixes in turn_complete_marker.rs were already present in the dirty tree from an interrupted prior session and were verified correct
    - test: green — rdeps(claude-agent) 6096/6096, rdeps(agent-client-protocol-extras) 6398/6398, fmt clean, clippy clean
    - commit: 84d5521ca
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-01T19:10:51.193682+00:00
- actor: claude-code
  id: 01kyzc2g75w86jjz2tkt0fkxg4
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit 84d5521ca) — 0 findings, 0 confirmed, 0 refuted, 16 validator tasks attempted, 0 failed. Diff was 3 files: two `.kanban` files for this card and `crates/claude-agent/tests/integration/turn_complete_marker.rs`; `agent_trait_impl.rs` carried no diff in this commit (byte-exact restore). All rounds 1-4 code findings are `[x]`. The one remaining `[ ]` (`crates/llama-agent/src/acp/server.rs` review-coverage gap) is a documented tooling limit tracked on ^3rnvage, not a code finding — rounds 2 and 4 already recorded it does not block `done`.
    - next: none — moved to `done`.
  timestamp: 2026-08-01T19:16:03.941677+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8a80
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
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean.


## Review Findings (2026-08-01 10:09)

Scope: commit `fcf1674b0` (`HEAD~1..HEAD`). Engine line numbers were resolved to
their true location and blame-checked against `fcf1674b0`; pre-existing findings
are dropped.

- [x] `crates/agent-client-protocol-extras/src/turn_complete.rs:1` — Module-level documentation for public APIs should include code examples demonstrating common use cases; the current documentation explains the architecture but lacks code examples showing how to create and check turn-complete markers. Add code examples to the module or function documentation demonstrating the two primary use cases: (1) creating a turn-complete marker notification with `turn_complete_notification()`, and (2) checking whether a notification is a marker with `is_turn_complete()`.
- [x] `crates/agent-client-protocol-extras/src/playback.rs:285` — String literal "session/update" is repeated — appears at line 285 and line 333, should be a named constant. Define const NOTIFICATION_METHOD: &str = "session/update" and use it in both locations.
- [x] `crates/claude-agent/src/lib.rs:75` — Public re-export from `agent_client_protocol_extras` lacks a doc comment. Add a doc comment above the re-export explaining the purpose of `is_turn_complete`, `turn_complete_notification`, and `TURN_COMPLETE_META_KEY`.
- [x] `crates/claude-agent/src/lib.rs:320` — pub struct NotificationCollector does not implement Debug, but all public types with non-empty representation must implement Debug for debuggability and to avoid downstream orphan-rule violations. Add #[derive(Debug)] to the struct on line 320.
- [x] `crates/claude-agent/src/lib.rs:391` — `matched_count` is incremented only in `process_notification`, but that function is skipped when the end-of-turn marker is detected. The marker is a notification for the collector's own session, so it should be counted, but the invariant (docstring: 'Notifications received so far for this collector's own session') is violated when marker notifications bypass the increment. Increment `matched_count` when `is_turn_complete(&notification)` is true at line 391, before the break, so that marker notifications are counted. Or update the docstring to clarify that `matched_count` only counts notifications processed by `process_notification`, not all session notifications.
- [x] `crates/claude-agent/src/lib.rs:472` — Error handling pattern repeated four times in `collect_response_content` error paths: get `collected_so_far`, call `tracing::error!()`, return `AgentError::Internal`. Structure is near-identical across lines 472, 486, 505, 520. Extract a helper function that encapsulates the common error reporting pattern. The function would take parameters for: (1) the error context/message, (2) which additional fields to log, and (3) the collected_text Mutex reference. This consolidates the error path logging and reduces maintenance burden.
- [x] `crates/claude-agent/src/lib.rs:486` — Error handling pattern identical to line 472: get `collected_so_far`, call `tracing::error!()`, return `AgentError::Internal`, repeated in the `Err(_elapsed)` timeout case. Extract as noted in the line 472 finding; parameterize the error context and logging fields.
- [x] `crates/claude-agent/src/lib.rs:520` — Error handling pattern identical to line 472: get `collected_so_far`, call `tracing::error!()`, return `AgentError::Internal`, repeated in the `if end == CollectorEnd::StreamClosed` case. Extract as noted in the line 472 finding; parameterize the error context and logging fields.
- [x] `crates/claude-agent/src/lib.rs:537` — Empty response content is logged at error severity but the function returns Ok, creating a contract mismatch. Error-level diagnostics signal failure conditions that callers should act on, but Ok return indicates success. A caller checking return value success cannot detect this error condition. Either (a) return `Err(AgentError::Internal(...))` when content is empty (if empty response is a failure), or (b) change the tracing level to `tracing::warn!` or `tracing::debug!` (if empty response is valid). Align diagnostic severity with the semantic intent.
- [x] `crates/claude-agent/src/lib.rs:588` — Unexplained buffer size `64` for NotificationSender; should be a named constant to document its purpose and allow reuse. Extract to a named constant (e.g., `const NOTIFICATION_SENDER_BUFFER_SIZE: usize = 64;`) at the top of the test module or file, and add a comment explaining why 64 is the right capacity for normal test operations (contrast with the intentional `2` in the lag test).
- [x] `crates/claude-agent/src/lib.rs:627` — Unexplained buffer size `64` for NotificationSender; should be a named constant to document its purpose and allow reuse. Extract to a named constant (e.g., `const NOTIFICATION_SENDER_BUFFER_SIZE: usize = 64;`) at the top of the test module or file.
- [x] `crates/claude-agent/src/lib.rs:654` — Unexplained buffer size `64` for NotificationSender; should be a named constant to document its purpose and allow reuse. Extract to a named constant (e.g., `const NOTIFICATION_SENDER_BUFFER_SIZE: usize = 64;`) at the top of the test module or file.

### Emitter gap the review engine missed (13th requirement)

- [x] `crates/claude-agent/src/agent_trait_impl.rs` — `ClaudeAgent::prompt` had two `?` operators BEFORE the split that guarantees the end-of-turn marker: `validate_prompt_request` and `resolve_session`. Either failure returned without emitting the marker, so a client with a live collector on that session waited out the full 10 s drain backstop and then errored — on a valid, already-subscribed session. Fixed by resolving the session FIRST (an unresolvable id names no notification stream, so there is nothing to mark) and moving request validation into `run_prompt_turn`, which every exit of leaves through `notify_turn_complete`. Regression test: `crates/claude-agent/tests/integration/turn_complete_marker.rs::a_rejected_prompt_ends_the_turn_for_a_subscribed_collector` — RED was `Elapsed(())` at a 2 s bound (the collector was on its way to the 10 s backstop), GREEN passes in ~4 s including agent construction.

### Review coverage gap (not a code finding)

- [ ] `crates/llama-agent/src/acp/server.rs` — the engine CANNOT review this file: it inlines 318,564 bytes, over the 262,144-byte review batch size, and a file is never split across batches. The `batch_size` modifier is ignored by the running MCP server (a review with `batch_size: 1` still reported the 262,144 default), and the `sah tool review` CLI path has no agent factory, so no route raises the budget. The 9 lines this commit adds at `server.rs:2705-2713` (the `AcpServer::prompt` end-of-turn marker emitter) therefore went un-reviewed. Split the file, or repair the `batch_size` passthrough, then re-review it.

## Review Findings (2026-08-01 12:35)

Scope: commit `d29a4ce25` (`HEAD~1..HEAD`), 18 files. `review sha` HARD-ERRORED
again on `crates/llama-agent/src/acp/server.rs` (319,069 bytes, over the
262,144-byte batch budget), so this pass used per-file `review file` runs, as the
previous pass did. `crates/llama-agent/src/acp/server.rs` could NOT be reviewed —
the same tooling limit, tracked by ^3rnvage. Its change in this commit is comments
only; the emitter code itself blames to `fcf1674b0`, out of this scope.

Engine line numbers were resolved to their true location and blame-checked against
`d29a4ce25`. 113 raw findings came back across the reviewed files; 111 blame to
commits between 2025-12-09 and 2026-05-01 and are pre-existing, so they are
dropped. Two remain.

- [x] `crates/agent-client-protocol-extras/src/playback.rs:290` — Near-verbatim pattern for sending session update notifications repeats in two functions. Both `send_recorded_notifications` (lines 289-291) and `send_turn_complete` (lines 319-324) construct `Request::notification_v2(SESSION_UPDATE_METHOD.to_string(), Some(params))` and send it via `send_message`. This pattern should be extracted into a shared helper to avoid drift if the structure or constants change. Extract a helper function `fn send_session_update_notification(tx: &..., params: Params) -> AcpResult<()>` that combines the `Request::notification_v2(SESSION_UPDATE_METHOD.to_string(), Some(params))` construction and `send_message` call. Call this helper from both `send_recorded_notifications` and `send_turn_complete` to eliminate the duplication.
  - Resolved locations: the construction sits at `playback.rs:288-292` inside `send_turn_complete`, and at `playback.rs:340-341` inside `send_recorded_notifications`. The engine swapped the two function names, and its second range is stale.
  - Guardrail: this is the SAME two call sites round 1 flagged at `playback.rs:285` and the same duplication rule, one level of abstraction up — round 1 asked for a shared constant, this asks for a shared helper. The two are additive, not contradictory.
  - FIXED: `send_session_update_notification(tx, params)` is the one construction site. Both callers go through it. Swept the file for the root: no other site builds a `session/update` notification by hand — the only remaining `notification_v2` calls are inside the helper and a test's `session/cancel`.
- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:1` — The module claims "Every `session/prompt` exit path emits the end-of-turn marker", but only ONE exit is tested: the rejected prompt. The two exits that the new `marker_session_id` fallback in `ClaudeAgent::prompt` exists to serve have no test — an unresolvable session id (`invalid_params`, not found) and a session store that cannot be read (`internal_error`, -32603). Those are exactly the exits where the marker is addressed to `request.session_id` instead of the resolved id, so the addressing rule the code documents is unverified. This card already claimed "every exit" once on unverified reasoning and was wrong three ways. Add one test per exit: subscribe a collector on the id the client sends, prompt with that id, and assert the drain ends on the marker inside `DRAIN_MUST_FINISH_WITHIN` rather than on the backstop.
  - FIXED: two tests added, one per exit, each subscribing on the id the CLIENT sends. The -32603 exit is staged by poisoning the `SessionManager` `RwLock` through its own public `update_session` with a panicking closure — the real condition, no test-only production hook. Both RED with the pre-fix `?` (`Elapsed(())` at 2.21 s, far below the 10 s backstop), both GREEN after. Neither spawns the claude CLI (`spawn_claude_on_new_session: false`); each runs in 0.21 s.
  - The module doc no longer overclaims. It now says what is proven — "every `session/prompt` exit that returns without running a turn" — tabulates the three exits with their error code and the id the marker is addressed to, and states plainly that a backend-driven turn is not tested here because it needs the CLI.

### Judgment on the open review-coverage item

The unchecked `crates/llama-agent/src/acp/server.rs` item does NOT block `done` for
this card. It is a tooling limit, its remedy (split a 319 KB file, or repair the
`batch_size` passthrough) is outside a card about a notification drain, and it is
tracked with its own acceptance criterion on ^3rnvage ("`crates/llama-agent/src/acp/server.rs`
gets a review through a normal route"). For THIS commit the un-reviewed delta is a
comment block, read by hand and correct. The card stays in `review` for the two
code findings above, not for this item.

## Review Findings (2026-08-01 13:06)

Scope: commit `359a19014` (`HEAD~1..HEAD`), 4 files — 2 code, 2 kanban. `review sha`
did NOT error this round. This commit does not touch
`crates/llama-agent/src/acp/server.rs`, so the 262,144-byte batch budget was never
reached and the whole range went through one normal run. The engine returned 7
findings (7 confirmed, 2 refuted).

Every engine line number was stale. Each was resolved to its true location and
blame-checked against `359a19014`. Four findings blame to older commits. They are
pre-existing and are dropped:

- `playback.rs:107` (`PlaybackAgent` needs `Debug`) — true line 83, blame `b3b00137af`, 2025-12-24.
- `playback.rs:113` (`new` should take `impl AsRef<Path>`) — true line 108, blame `b3b00137af`, 2025-12-24.
- `playback.rs:148` (`-32603` needs a constant) — true lines 233 and 361, blame `71441e9127`, 2026-05-01.
- `playback.rs:312` (`20` ms needs a constant) — true line 585, blame `71441e9127`, 2026-05-01. That line is also inside `#[cfg(test)] mod tests`, so the existing-test exception covers it as well.

The engine raised NOTHING against `send_session_update_notification`, which is the
code this commit adds to `playback.rs`. The round-3 duplication finding is closed.

Three findings remain. All three name one cause in one file.

- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:72` — 2 is a hardcoded worker thread count that configures test behavior and appears in multiple tests without explanation. Define `const TEST_WORKER_THREADS: usize = 2;` as a module constant and document why 2 threads are needed.
- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:143` — 2 is a hardcoded worker thread count that repeats across tests without naming. Use the same named constant as line 72 to avoid duplication.
- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:176` — 2 is a hardcoded worker thread count that repeats across tests without naming. Use the same named constant as line 72 to avoid duplication.
  - Resolved locations: the three `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` attributes are at lines 152, 191 and 230. All three blame to `359a19014`. Lines 152 and 191 belong to the two NEW tests. Line 230 is the pre-existing rejected-prompt test, which this commit moved; its attribute already read `worker_threads = 2`, so the existing-test exception covers that site alone. The other two are new code and stay in scope.
  - HALF OF THIS REMEDY CANNOT COMPILE. The `tokio::test` attribute macro reads `worker_threads` at expansion time. In `tokio-macros-2.7.1/src/entry.rs` the argument value must match `syn::Expr::Lit`, and anything else returns the error "Must be a literal"; `parse_int` then accepts only `syn::Lit::Int`. A constant path such as `TEST_WORKER_THREADS` is a `syn::Expr::Path`, so `worker_threads = TEST_WORKER_THREADS` does not compile. The named-constant part of the remedy is impossible, and the duplication that the second and third findings name cannot be removed this way.
  - RESOLVED: the named-constant half stays impossible for the stated macro reason; the module now carries a `# Why worker_threads = 2` doc section next to the three attributes that explains why the multi-threaded flavor is needed (the collector is a separately-polled `tokio::spawn`ed task, so 2 workers let it and the agent's own tasks run at the same time instead of in turn) and records the `syn::Expr::Lit`-only macro limitation so a later round does not re-ask for the constant.

### Reviewer assessment recorded as a requirement — the module doc still claims more than the three tests prove

The engine did not raise this. It answers a question the caller asked directly, so
it is a requirement here, in the same way the round-2 emitter gap was.

- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:17-21` — the "marker addressed to" column is proven for one row only. In row 1 the id names no session, so "the id the client sent" is the only address that can exist, and the passing drain proves it. In row 2 and row 3 the test sends the canonical id that `new_session` returned, so "the id the client sent" and "the resolved session id" are the SAME string. Those two rows cannot tell the two addressing rules apart, so the distinction the table draws between them stays unproven. Say in the doc that rows 2 and 3 do not discriminate the address, or make the client-sent id differ from the canonical id where that is possible.
  - RESOLVED: the table lost its "addressed to" column — it now lists only `exit` and `result`. Addressing is explained separately in a `# Which id the marker is addressed to` section, which states plainly that only the unresolvable-id test can tell the two addressing rules apart, because in every other test the client's id and the resolved id are the same string.
- [x] `crates/claude-agent/tests/integration/turn_complete_marker.rs:13-15` — "the three that return a `prompt` error before any backend work" reads as a complete list. It is complete only under the narrow reading "before any turn output is streamed". After `send_user_message_chunks`, `run_prompt_turn` returns a `prompt` error before the model runs at three more places: `prepare_session_for_turn(&session_id, &prompt_text)?`, `check_turn_limits(&session_id, &prompt_text)?` and `get_updated_session(&session_id)?`. It also returns `Ok` without running a turn at two more places — the pre-cancelled session and the turn limit. The headline sentence covers all five, and no test pins any of them. State which reading "before any backend work" takes, or remove the count.
  - RESOLVED: the module doc now states the exact reading — three tests cover every exit reachable before `run_prompt_turn` calls `send_user_message_chunks` — and names that set as complete under that reading. A fourth test, `a_pre_cancelled_session_ends_the_turn_for_a_subscribed_collector`, was added for the pre-cancelled `Ok(StopReason::Cancelled)` exit immediately past that point, since it needs no claude CLI. A new `# What these tests do NOT pin` section lists the four exits that remain unpinned by name — `prepare_session_for_turn`, `check_turn_limits`, `get_updated_session` (all three `prompt`-error exits) and the turn-limit `Ok` exit — and says plainly that driving a turn into the backend needs the claude CLI, which this module deliberately never spawns.

The headline sentence itself is correct and was verified: `ClaudeAgent::prompt`
holds no `?` and no early `return` before `notify_turn_complete`, so every exit
passes through the one emit.

### Assessment — poisoning the `RwLock` to stage the -32603 exit is accepted

The approach is sound, and it is accepted.

- It drives the real branch. `SessionManager.sessions` is a `std::sync::RwLock`. `update_session` calls the updater while it holds the write guard, so a panic in the closure unwinds through the guard and poisons the lock. That is guaranteed `std::sync` behavior, not a local accident. `get_session` then maps the `PoisonError` to `AgentError::Session`, and `resolve_session_with` maps that to `internal_error` (-32603).
- It is not a stand-in for the real cause. The production doc on `resolve_session_with` names lock poisoning as the cause of that branch, word for word: "A session-store FAILURE (lock poisoning) is a retryable `-32603` internal error instead". The test reproduces the documented condition, not an analogue of it.
- It uses only the public API — `session_manager()`, `update_session`, `get_session`. No test-only hook was added to production code.
- It cannot silently stop reproducing. Two asserts guard it. `assert!(panicked.is_err())` fails if the closure ever stops panicking — for example if the session lookup inside `update_session` misses, because the updater only runs `if let Some(session) = sessions.get_mut(session_id)`. `assert!(manager.get_session(&internal_id).is_err())` then proves the store is unreadable BEFORE the prompt is sent. Neither depends on inferring the state from the prompt's error code, so a broken setup is a loud failure, never a green test on the wrong path.
- The coupling is real but it fails loudly. The technique needs a poisoning lock. Move `sessions` to `tokio::sync::RwLock` or `parking_lot::RwLock` and nothing poisons, so `get_session` succeeds and the second assert fails. Whoever does that migration gets a red test that says the -32603 branch has no reachable fault injection left. That is useful information, not a false alarm.
- It needs unwinding. No profile in this workspace sets `panic = "abort"`, so `std::thread::spawn` plus `join()` returns `Err` as the test expects.
- One caveat, not a finding: `poison_the_session_store` swaps the PROCESS-global panic hook. `.config/nextest.toml` is present and nextest runs one process per test, so the swap is isolated. Under plain `cargo test` the three tests share one binary and run concurrently, and a genuine panic in a sibling test inside the swap window would lose its backtrace. That degrades diagnosis; it cannot turn a failure into a pass.

### Guardrail — repeat check for round 4

No new finding repeats an earlier round on the same file with the same cause.

- `turn_complete_marker.rs` and "a magic number needs a named constant": FIRST appearance on this file.
- `turn_complete_marker.rs` and "the module doc claims more than the tests prove": SECOND appearance. Round 3 raised it against the words "Every `session/prompt` exit path". This round raises it against the table and against the count of three. Two, not three, so the card is not stuck on it — but a third would mean it is.
- `playback.rs` and "a magic literal needs a named constant": round 1 asked for a `session/update` constant; this round names `-32603` and `20`. That is a second appearance on the file. Both new instances are pre-existing code and are dropped, so neither becomes work on this card.

Pattern to watch: the "unexplained literal needs a named constant" rule has fired in
2 of the 3 recorded rounds — 4 times in round 1, 5 times in round 4 — each time
against whatever file the newest commit touched. It is not a stuck loop yet. It will
become one if every round adds test code that holds a bare literal.

### The open review-coverage item still does not block `done`

The round-3 judgment stands, and this round supports it. `crates/llama-agent/src/acp/server.rs`
is a tooling limit tracked on ^3rnvage with its own acceptance criterion. This
commit does not touch the file, which is exactly why `review sha` ran clean through
the whole range this time. The card stays in `review` for the findings above, not
for that item.

## Review Findings (2026-08-01 14:11)

Scope: commit `84d5521ca` (`test(claude-agent): address round-4 findings on ^8ep9cnf`,
`HEAD~1..HEAD`). `git diff --stat HEAD~1..HEAD` shows 3 files changed: the two
`.kanban` task files for this card, and
`crates/claude-agent/tests/integration/turn_complete_marker.rs` (143 lines
changed). `crates/claude-agent/src/agent_trait_impl.rs` carries NO diff in this
commit — the round-4 restore described in the prior comment made it byte-identical
to the already-committed version, so there was nothing to commit for that file.
This commit does not touch `crates/llama-agent/src/acp/server.rs`, so the
262,144-byte batch-budget error did not recur; `review sha` ran the whole range in
one pass.

`review sha HEAD~1..HEAD` returned **zero findings** (0 confirmed, 0 refuted, 16
validator tasks attempted, 0 failed).

No new checklist items — nothing to blame-check.

### Verdict

Every code finding from rounds 1-4 is checked `[x]`. The one item still
unchecked, `crates/llama-agent/src/acp/server.rs` under "Review coverage gap (not
a code finding)", is a documented tooling limit, not a code finding — it is
tracked with its own acceptance criteria on ^3rnvage, this commit does not touch
that file, and rounds 2 and 4 already recorded that it does not block `done` for
this card. With a clean engine pass and no other open item, this card moves to
`done`.
#review #test-failure