---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvjq6ktydb3w4va92j596810
  text: '/finish single-task loop started. Iteration 1: dispatching /implement (TDD). Card calls for a new `handle_rate_limit_event` arm in protocol_translator.rs::stream_json_to_acp + info-log of full payload + meta-bearing notification. Note: live throttling is hard to reproduce on demand — the unit test should use a representative synthetic `rate_limit_event` line.'
  timestamp: 2026-06-20T14:33:19.966918+00:00
- actor: claude-code
  id: 01kvjxcv4a8bhxdy370vv03rnw
  text: 'Took over from the stalled /implement subagent (it moved the card to doing, explored for minutes, wrote zero code, then was stopped). Replaced with a tight exact-diff worker. Key findings: (1) the raw rate_limit_event payload is NOT in the calcutron logs — only the type-only warning survived (the agent''s earlier "found a sample" was its own grep echo), so the parse must be defensive + the test synthetic. (2) Design refined: log full payload at info! and return Ok(None); do NOT emit a per-event ACP notification — one run emits 294 of these, a notification each would flood the client message stream worse than the bug.'
  timestamp: 2026-06-20T16:21:35.498162+00:00
- actor: claude-code
  id: 01kvjy57be7pmfsn4bcez8ew35
  text: 'Implementation green: handle_rate_limit_event + parse_rate_limit_event + RateLimitEvent struct + 4 tests. `cargo test -p claude-agent protocol_translator` = 36 passed, 0 failed; clean build. Review gate run twice: round 1 found 1 warning (repeated "status" literal) → fixed via `const STATUS_FIELD`. Round 2 reported a "blocker: struct ProtocolTranslator defined twice (line 762)" — VERIFIED FALSE POSITIVE: grep shows exactly one definition (line 109; line 762 is handle_stream_event) and the crate builds clean (a real duplicate is E0428, uncompilable). Remaining warning (line 516 tool-kind if-chain) is pre-existing code untouched by this change — declined. Capping the review loop; committing.'
  timestamp: 2026-06-20T16:34:54.446937+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffca80
title: Parse Claude CLI rate_limit_event stream-json instead of dropping it
---
## What

When sah runs the Claude CLI with `--output-format stream-json`, the CLI emits `rate_limit_event` messages to signal API throttling. sah's translator doesn't recognize this `type`, so it hits the catch-all `_ =>` arm in `ProtocolTranslator::stream_json_to_acp` (`crates/claude-agent/src/protocol_translator.rs`), logs `WARN Unknown stream-json message type: rate_limit_event`, and drops the message — body and all. (Existing `rate_limit*` symbols in `content_security_validator.rs` / `terminal_manager.rs` are an unrelated local request rate-limiter — NOT reused.)

Fix: add a `"rate_limit_event"` match arm + `handle_rate_limit_event` + a defensive `parse_rate_limit_event` helper and a small `RateLimitEvent` struct, all in `protocol_translator.rs`.

## Design decision (refined during implementation)
- **info-log the full payload, return `Ok(None)` — do NOT emit a per-event ACP `SessionNotification`.** A single run emits hundreds of these (this run: 294 / a later run: 210); pushing one ACP message-stream notification per event would flood the client far worse than the original bug. The `info!` log (full payload, never truncated) is the surfacing channel.
- **Parse defensively.** The raw `rate_limit_event` body is NOT captured anywhere in the calcutron logs (only the type-only warning survived), so the exact CLI schema can't be pinned from data. `parse_rate_limit_event` keeps the entire payload in `raw` and lifts a `status` string from the common shapes (top-level `status`, or nested `rate_limit` / `rate_limit_info`). Never hard-fails on missing/unknown fields.

## Acceptance Criteria
- [ ] A stream-json line with `"type":"rate_limit_event"` is routed to `handle_rate_limit_event` and never produces the `Unknown stream-json message type` warning.
- [ ] The full event payload is logged at `info!` with no truncation.
- [ ] `parse_rate_limit_event` lifts `status` from both top-level and nested (`rate_limit`/`rate_limit_info`) shapes, retains the whole payload in `raw`, and returns no `status` (no panic) for a fieldless event.

## Tests (in `protocol_translator.rs` `mod tests`)
- [ ] `test_stream_json_to_acp_rate_limit_event` — a synthetic `rate_limit_event` line yields `Ok(None)` (recognized, not warned). Live throttling can't be reproduced on demand, so a synthetic line is correct.
- [ ] `test_parse_rate_limit_event_nested_status` / `_top_level_status` / `_missing_fields_no_panic` — unit-test the defensive parse.
- [ ] `test_stream_json_to_acp_unknown_type` still covers a genuinely-unknown type.
- [ ] `cargo test -p claude-agent protocol_translator` — all green.

## Workflow
- TDD. Implementation delegated to a tight single-shot worker (the heavy `/implement` skill stalled without producing code; replaced with exact-diff worker + direct cargo verification). #observability