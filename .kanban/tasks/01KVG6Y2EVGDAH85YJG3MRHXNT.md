---
assignees:
- claude-code
comments:
- actor: wballard
  id: 01kvgbzkqvkyfgk7gnv1615tek
  text: |-
    Picked up. Research done — verified all anchors:
    - protocol_translator.rs: StreamResult (~30), parse_result_message (~816), "result" case Ok(None) (~195), tests mod at ~1411 in protocol_translator? (actually tests in same file around line 1215).
    - claude.rs: MessageChunk struct (651-668, has stop_reason field), send_final_chunk (1152) builds the final chunk from result.stop_reason.
    - agent_prompt_handling.rs: process_stream_chunks collects chunk.stop_reason (336-388), build_streaming_response stuffs a serde_json::Map into PromptResponse.meta (737-756). PromptResponse.meta is Option<Meta> (derefs to a JSON map with .get()), set via .meta(map) builder.
    - lib.rs: CollectedResponse (108), constructed at 224 from prompt_response.

    Plan: stash cache_usage as a JSON object in PromptResponse.meta under key "cache_usage" (no typed field on the ACP schema type), read it back in lib.rs to populate CollectedResponse.cache_usage. CollectedResponse-level test uses the existing process_stream_chunks harness (fixed_text_stream pattern) driving a final chunk carrying CacheUsage, asserting response.meta["cache_usage"]. Note: existing test_collected_response_debug in coverage_tests.rs constructs CollectedResponse literally — must add the new field there.

    Writing failing tests first (TDD).
  timestamp: 2026-06-19T16:38:47.291017+00:00
- actor: wballard
  id: 01kvgcp8zb4h46n5ak3crmsayk
  text: |-
    Implemented (TDD). RED→GREEN evidence:
    - RED: added the two parse_result_message tests + the process_stream_chunks cache_usage test; ran them → compile errors (CacheUsage / StreamResult.cache_usage / MessageChunk.cache_usage missing). Feature-missing failure confirmed.
    - GREEN: 8 targeted tests PASS (the new 3 + the 5 unchanged stop_reason tests).

    Files changed:
    - protocol_translator.rs: new CacheUsage struct (Copy/Default/Eq) with META_KEYS + symmetric to_meta_json/from_meta_json; StreamResult.cache_usage; parse_result_message populates via parse_cache_usage→from_meta_json. Empty/absent usage → None.
    - claude.rs: MessageChunk.cache_usage field; send_final_chunk rides result.cache_usage alongside stop_reason; all in-crate MessageChunk literals updated.
    - agent_prompt_handling.rs: process_stream_chunks collects chunk.cache_usage; build_streaming_response takes it and stashes usage.to_meta_json() in PromptResponse._meta under "cache_usage". New test_process_stream_chunks_carries_cache_usage_to_meta.
    - lib.rs: CollectedResponse.cache_usage; populated at construction by reading prompt_response.meta["cache_usage"] via CacheUsage::from_meta_json.
    - coverage_tests.rs: updated CollectedResponse literal.

    Double-check (advisory) flagged one real issue: CollectedResponse is not #[non_exhaustive] and validators/pool.rs:276 constructs it, so the new required field broke that crate's build. Applied a mechanical compile-fix only (cache_usage: None with a comment that threading real data is the follow-up task) — NOT the observability feature work. Round-trip symmetry, empty→None invariant, and unchanged stop_reason all PASSed the critique.

    Verification:
    - cargo nextest run -p claude-agent → 1061 passed, 0 failed.
    - cargo nextest run -p swissarmyhammer-validators → 306 passed, 0 failed.
    - cargo clippy -p claude-agent -p swissarmyhammer-validators --all-targets -- -D warnings → clean (exit 0).
  timestamp: 2026-06-19T16:51:09.931714+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc680
project: local-review
title: 'claude-agent: surface Anthropic prompt-cache usage on CollectedResponse'
---
## What

The `claude` CLI reports per-turn prompt-cache metrics in the stream-json `result` message's `usage` object — `cache_read_input_tokens` (warm reuse), `cache_creation_input_tokens` (cold write), plus `input_tokens` / `output_tokens`. Today the translator **discards all of it**: `ClaudeStreamTranslator::stream_json_to_acp` matches `"result"` and returns `Ok(None)` with a "metadata only" debug log (`crates/claude-agent/src/protocol_translator.rs:195-198`), and `parse_result_message` (`protocol_translator.rs:816`) extracts only `stop_reason`. As a result every downstream consumer — notably the review fleet — is blind to whether a Claude turn was a warm cache read or a cold prefill.

This task captures those fields and exposes them on `CollectedResponse`. (Threading them into the review fleet's per-task log is the dependent follow-up — see the linked task; do NOT touch `swissarmyhammer-validators` here.)

Plumbing (additive; the `stop_reason` path is the template — ride alongside it):
1. Add a small `CacheUsage` struct (in `protocol_translator.rs` near `StreamResult`):
   ```rust
   #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
   pub struct CacheUsage {
       pub cache_read_input_tokens: Option<u64>,
       pub cache_creation_input_tokens: Option<u64>,
       pub input_tokens: Option<u64>,
       pub output_tokens: Option<u64>,
   }
   ```
2. Extend `StreamResult` (`protocol_translator.rs:30`) with `cache_usage: Option<CacheUsage>` and populate it in `parse_result_message` by reading `parsed["usage"]` (each field via `.get(..).and_then(JsonValue::as_u64)`). An empty `"usage":{}` → all-`None` fields → store `None` (no cache info) rather than a zeroed struct.
3. Carry it through the final streamed chunk in `crates/claude-agent/src/claude.rs:1152-1163` — the chunk already carries `result.stop_reason`; add a `cache_usage` field to that chunk struct and set it from `result.cache_usage`.
4. In `crates/claude-agent/src/agent_prompt_handling.rs` (~336-388), collect the chunk's `cache_usage` alongside `claude_stop_reason`, and carry it onto the `PromptResponse` produced by `build_streaming_response` (~740-755). Prefer a typed carry; if `PromptResponse` has no field for it, stash it in the `_meta` map under a `cache_usage` key (JSON object) — whichever keeps it retrievable in step 5.
5. Add `cache_usage: Option<CacheUsage>` to `CollectedResponse` (`crates/claude-agent/src/lib.rs:108-112`) and populate it at the construction site (`lib.rs:224`) from the prompt response.

Out of scope: the llama/qwen path (it already reports `prefix_tokens` via native KV fork); any change in `swissarmyhammer-validators`; emitting a new ACP notification (keep `result` returning `Ok(None)` from `stream_json_to_acp` — this is a typed-extraction change, not a protocol change).

## Acceptance Criteria
- [ ] `parse_result_message` populates `StreamResult.cache_usage` with `cache_read_input_tokens` and `cache_creation_input_tokens` (and input/output) parsed from the result message's `usage` object.
- [ ] A result message with `"usage":{}` (or no `usage`) yields `cache_usage: None` and does not panic.
- [ ] `CollectedResponse` exposes the parsed `cache_usage`, populated from a streamed turn that ended with a `result` message carrying `usage`.
- [ ] Existing `stop_reason` extraction and the `test_parse_result_message_*` tests are unchanged and still pass.

## Tests
Add to the existing `#[cfg(test)] mod tests` in `crates/claude-agent/src/protocol_translator.rs` (which already has `test_parse_result_message_with_max_tokens` etc.):
- [ ] `test_parse_result_message_extracts_cache_usage`: line `{"type":"result","subtype":"success","stop_reason":"end_turn","usage":{"cache_read_input_tokens":1234,"cache_creation_input_tokens":56,"input_tokens":1290,"output_tokens":42}}` → `StreamResult.cache_usage == Some(CacheUsage{ cache_read_input_tokens: Some(1234), cache_creation_input_tokens: Some(56), input_tokens: Some(1290), output_tokens: Some(42) })`.
- [ ] `test_parse_result_message_empty_usage_is_none`: line with `"usage":{}` → `cache_usage == None`; stop_reason still parsed.
- [ ] A `CollectedResponse`-level test (use the existing claude-agent streaming test harness / fake stdout that drives `collect`-style assembly, mirroring how stop_reason is already tested) asserting `collected.cache_usage` reflects the `usage` from the final `result` line.
- [ ] Run: `cargo nextest run -p claude-agent` — all green (cargo nextest is mandated, never bare cargo test). Also `cargo clippy -p claude-agent --all-targets -- -D warnings` clean.

## Workflow
- Use `/tdd` — write the failing `parse_result_message` + `CollectedResponse` tests first, watch them fail, then thread the field through to green.