---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky54e3zyr8qf2x1dwg8p4rqd
  text: '/finish picked up ^mnchzmg (single-task mode), iteration 1. Plan: /implement → /test → /commit checkpoint → /review HEAD~1..HEAD. Root cause already traced in the card: spawn_review_progress_bridge bails on missing progressToken, killing notifications/message and the keep-alive along with progress ticks.'
  timestamp: 2026-07-22T14:42:20.798161+00:00
- actor: claude-code
  id: 01ky54v9bw2dt9jyk05dgw94hk
  text: |-
    Picked up, moved to doing. Research done; two findings that shape the implementation:

    1. Card locations verified: spawn_review_progress_bridge review_op.rs:748 bails on `context.progress_token.clone()?`; keep-alive const at 809; mapping loop at 824; unit tests at 1473/1480; call-site comment mod.rs:355-359; stdio e2e at review_progress_stdio_test.rs:180.

    2. IMPORTANT deviation from the card's test sketch: rmcp 1.7's client `send_request_with_option` (service.rs:463-466) UNCONDITIONALLY injects a progressToken into `_meta` on every outgoing request — an rmcp client cannot make a tokenless call. (This is exactly why the existing rmcp e2e stayed green while Claude Code, which omits the token, failed in the field.) The new integration test therefore drives the same real McpServer over the same duplex byte-stream transport with a RAW newline-delimited JSON-RPC client (initialize handshake + tools/call with no _meta), asserting client-side frame receipt: notifications/message count > 0, notifications/progress count == 0. Same production boundary, honest tokenless call.

    Design: extract a data-driven ReviewBridgePlan (token/sink/peer → None | SinkProgressOnly | PeerProgressAndContent | PeerContentOnly) consulted by spawn_review_progress_bridge; content events flow through a new content channel + content drain task to the peer (mirrors the progress drain, gives the paused-time unit test a real seam); run_review_progress_mapping takes Option<ProgressToken> + Option<content_tx>, keep-alive re-arms from the last WIRE SEND on either channel (not the last engine event — tokenless progress ticks produce no traffic and must not silence the timer), and fires the latest progress param when one exists, else a {"kind":"review.keep-alive"} notifications/message. WARN on peer-without-token. drive.py gets notification counting + tokenless assertions + self-test cases.
  timestamp: 2026-07-22T14:49:32.284664+00:00
- actor: claude-code
  id: 01ky5607p4ncx91t87neevh72b
  text: |-
    Implementation complete and green (TDD: RED integration test watched failing at exactly "got zero notifications/message" before the fix). Left in doing for /review.

    What changed:
    - review_op.rs: new data-driven ReviewBridgePlan (token/sink/peer → Nothing | SinkProgressOnly | PeerProgressAndContent | PeerContentOnly) consulted by spawn_review_progress_bridge; new spawn_content_drain_task mirroring the progress drain; run_review_progress_mapping now takes Option<ProgressToken> + Option<content_tx>, keep-alive re-arms from the last WIRE SEND on either channel and falls back to a {"kind":"review.keep-alive"} notifications/message when no progress param exists; WARN on peer-without-token; bridge drain is one handle joining both drains. Unit tests updated/added: no_transport_at_all_means_no_bridge, bridge_plan_streams_content_to_a_peer_without_a_token (full 8-row plan table), tokenless_mapping_streams_content_and_bounds_message_gaps_by_the_keep_alive (paused time), take_buffered made generic; 5 existing mapping tests updated to Some(token).
    - mod.rs: call-site comment rewritten (old "no token → zero notifications" doc removed).
    - description.md: Streaming section now documents the per-channel gates + review.keep-alive kind.
    - tests/review_progress_stdio_test.rs: new review_content_is_streamed_to_a_client_that_omits_the_progress_token — RAW newline-delimited JSON-RPC client over the same duplex transport (rmcp client cannot omit the token, see earlier comment); asserts review-logger notifications/message > 0 and notifications/progress == 0.
    - scripts/review-verify/drive.py: reader tallies notifications; check_streamed_notifications (messages>0, progress==0) + 3 self-test cases; PLUS two pre-existing staleness fixes it needed to pass at all: (1) review file target must be repo-RELATIVE now (scope validator rejects absolute paths as "escapes the repository root"), (2) counts assertion read the removed blockers/warnings/nits keys — now reads the serialized `findings` count.

    Evidence (all fresh runs):
    - cargo nextest run -p swissarmyhammer-tools review_progress → 3/3 pass
    - targeted run of everything review-related (57 tests incl. both e2e binaries + all review_op unit tests) → 57/57 pass
    - cargo fmt --all applied; cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings → clean
    - python3 scripts/review-verify/drive.py --self-test → 13/13 PASS
    - rebuilt sah (cargo install) and ran FULL python3 scripts/review-verify/drive.py → PASS, exit 0: real qwen model, tokenless call, client received 9 notifications/message (logger "review") and 0 notifications/progress; server log contains the new WARN exactly once (mcp.97777.log).
    - double-check adversarial pass: VERDICT PASS.
  timestamp: 2026-07-22T15:09:42.980255+00:00
- actor: claude-code
  id: 01ky5623k59vzb2f8wm5y0cez1
  text: 'Iteration 1 implementation landed (verified against git diff: 5 files, +642/−108). Key discovery from the implementer: rmcp 1.7 clients unconditionally inject a progressToken (service.rs next_progress_token), so a tokenless call is impossible through an rmcp client — which is why the pre-existing e2e stayed green while Claude Code subagent clients failed in the field. New integration test uses a raw newline-delimited JSON-RPC client over the duplex stream to actually exercise the tokenless path (TDD RED confirmed pre-fix). Full drive.py e2e passed: 9 notifications/message, 0 notifications/progress, WARN present exactly once. Now running the /test gate on the changed crate.'
  timestamp: 2026-07-22T15:10:44.325654+00:00
position_column: doing
position_ordinal: '8280'
title: 'review: clients that omit progressToken get zero notifications and time out — decouple notifications/message from the token gate'
---
# review: tokenless MCP clients get total silence

## What

Field report: a subagent ran `review working` against its own `sah serve` (stdio) — the engine ran to completion (server logs showed 14/14 pairs, 518 notifications' worth of events flowing internally) but the subagent's MCP client received **zero** `notifications/progress` and **zero** `notifications/message`, so it sat silent and hit its 30-minute client tool-timeout twice. The main session's connection receives everything.

Root cause (traced, not routing): review notifications are strictly per-request-peer and in-process — leader election never touches review dispatch. The entire streaming pipeline hangs on one line: `spawn_review_progress_bridge` in `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` (~line 748) begins `let token = context.progress_token.clone()?;` — a `tools/call` without `_meta.progressToken` gets **no bridge at all**. That kills both notification kinds AND the 10s keep-alive (`REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL`, review_op.rs ~809) that exists precisely to hold client timeouts open. Subagent Claude Code connections evidently omit the token; the peer is always present.

Per MCP spec only `notifications/progress` requires a client-supplied token. `notifications/message` (logger "review": `review.findings` / `review.verdict`, built by `review_content_log_param` and sent by `send_review_content_log`, review_op.rs ~656–706) does not — gating it on the token is the bug.

Fix in `review_op.rs` (+ call-site comment in `tools/review/mod.rs` ~358–360 which documents the current "no token → zero notifications" behavior as intended):
- [x] Split `spawn_review_progress_bridge`: build the bridge whenever the context has a peer (or in-process sink). Progress ticks (`notifications/progress` via the `progress.rs` drain task) stay token-gated; content events (`notifications/message`) always flow to the per-request peer.
- [x] Arm the keep-alive re-send on the content channel too, so a tokenless client still sees traffic within 10s of the first event and its timeout keeps resetting.
- [x] Emit one WARN when a review runs with a peer but no progressToken, stating progress ticks are disabled for this call.
- [x] Update the unit tests that pin the old behavior (`no_progress_token_means_no_bridge`, `a_token_without_peer_or_sink_means_no_bridge`, review_op.rs ~1473–1483) to assert the new contract: peer-without-token → bridge with content-only streaming.
- [x] Production-boundary e2e: extend `scripts/review-verify/drive.py` (which spawns a real `sah serve` stdio subprocess but currently sends no token and asserts nothing about notifications) to assert client-side receipt of `notifications/message` on a tokenless call.

## Acceptance Criteria

- [x] A real MCP client calling `review file` with a peer but **no** `_meta.progressToken` receives `notifications/message` events (logger "review") for findings/verdicts, and receives **no** `notifications/progress`.
- [x] With a token supplied, both notification kinds arrive exactly as today (existing streaming tests stay green).
- [x] During a tokenless review, the gap between consecutive `notifications/message` sends after the first event never exceeds the keep-alive interval (paused-time unit test on `run_review_progress_mapping`).
- [x] Server log contains the new WARN for tokenless-with-peer reviews.

## Tests

- [x] New integration test in `crates/swissarmyhammer-tools/tests/review_progress_stdio_test.rs`: real rmcp client over the byte-stream transport, no progressToken, asserts `on_logging_message` receipt > 0 and `on_progress` receipt == 0 (mirror of the existing `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport` at ~line 179). Fails before the fix (regression test). NOTE: implemented as a raw newline-delimited JSON-RPC client over the same duplex transport, because rmcp 1.7's client unconditionally injects a progressToken into `_meta` on every request — a tokenless call is only producible below the rmcp client layer.
- [x] Updated unit tests in `review_op.rs` for the new bridge contract, including a paused-time keep-alive test on the content channel.
- [x] `cargo nextest run -p swissarmyhammer-tools review_progress` — all pass.
- [x] `python3 scripts/review-verify/drive.py` — passes and now asserts tokenless `notifications/message` receipt over a real spawned `sah serve` subprocess.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.