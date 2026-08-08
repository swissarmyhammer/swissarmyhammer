---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgaay20tec27dz2aq18c6v7
  text: |-
    Research done. The card's cited line numbers came from the `../swissarmyhammer-main` checkout and no longer match this branch, but the defect is real. Corrected root cause:

    - The STREAMING path is already fixed (commit a067b678c, 2026-06-19). `agent_prompt_handling::build_streaming_response` already does `meta_map.insert("cache_usage", usage.to_meta_json())`.
    - The review fleet never runs the streaming path. `review/drive.rs::run_pipeline_in_connection` sends `InitializeRequest::new(1.into()).client_capabilities(ClientCapabilities::new().fs(...))` — no `.meta(...)`, so `client_capabilities.meta` is `None`.
    - `agent_prompt_handling::should_stream` reads `client_capabilities.meta["streaming"]` and `unwrap_or(false)`. So every pool turn takes `handle_non_streaming_prompt`.
    - `process_non_streaming_chunks` never reads `chunk.cache_usage`, and `handle_non_streaming_prompt`'s final meta carries `processed`, `streaming`, `claude_response`, `session_messages` — no `cache_usage`. The value dies there.
    - `claude.rs::send_final_chunk` DOES put `result.cache_usage` on the last `MessageChunk` for both paths, so the data reaches the non-streaming loop and is dropped.

    The pool side (`validators/pool.rs`, `review/fleet/prime.rs::classify_reuse`) is complete and needs no change.

    Fix: the non-streaming path must keep the last non-empty `chunk.cache_usage` and insert it into the response `_meta` under the same `cache_usage` key, sharing one helper with the streaming path so the two cannot drift.

    Test plan: a production-path test with a scripted `claude` executable on PATH (the prevailing pattern already used by `claude.rs::spawn_process_and_consume_init_skips_trigger_when_configured`, with `PathGuard` + `#[serial_test::serial(path_env)]`), driving a real `ClaudeAgent` through initialize -> new_session -> prompt and asserting `_meta.cache_usage.cache_read_input_tokens`.
  timestamp: 2026-08-08T09:12:48.448469+00:00
- actor: claude-code
  id: 01kzgcan3r5s64kcwvqgfsj8tc
  text: |-
    Implementation landed.

    What changed:
    - `protocol_translator.rs` — new `CacheUsage::META_KEY` ("cache_usage"). One name for the wire key, shared by every writer and reader so the two sides cannot drift.
    - `agent_prompt_handling.rs` — two shared helpers: `fold_cache_usage` (last non-empty `usage` on the turn wins — the card's requirement 2) and `attach_cache_usage` (writes `META_KEY` into a response `_meta`, writes nothing when the turn reported no usage).
    - Streaming path now folds and attaches through those helpers instead of its own inline copies. Behaviour unchanged.
    - Non-streaming path — the real gap. `process_non_streaming_chunks` now folds `chunk.cache_usage` and returns it on `NonStreamingChunkOutcome::Finished`; `handle_non_streaming_prompt` attaches it to the `EndTurn` meta.
    - Same function, two sibling terminal responses that also had the usage in hand and dropped it: the refusal response (`ClaudeAgent::create_refusal_response` gained a `cache_usage` parameter) and the cancelled-after-API-response reply. Both now attach. Leaving them out would put the same "every fork ran cold" lie back into the fleet log for refused and late-cancelled tasks.
    - `lib.rs` and `validators/pool.rs` read through `META_KEY` instead of a literal.

    Tests (TDD, both watched RED first):
    - `a_warm_non_streaming_turn_reports_cache_usage_on_the_response_meta` — production path. A scripted `claude` executable on PATH answers the init handshake and one turn, ending in a `result` line whose `usage` reports `cache_read_input_tokens: 68324`. Drives a real `ClaudeAgent` through initialize -> new_session -> prompt with no `streaming` client capability (what the fleet sends), asserts `_meta.streaming == false` and `_meta.cache_usage.cache_read_input_tokens == 68324`. RED before the fix: `got {"claude_response": "ok", "processed": true, "session_messages": 2, "streaming": false}` — no cache_usage.
    - `a_refused_turn_still_reports_its_cache_usage` — same harness, refusal reply. RED before the fix: `got {"refusal_detected":true,"session_id":"..."}`.
    - `PathGuard` moved out of `claude.rs`'s test module into `crate::test_support` so both files share one copy. Its serial key changed from `path_env` to the default `#[serial]` group — the group `StateDirGuard` callers use — so a PATH mutator and an XDG_STATE_HOME mutator can never interleave.
    - The existing streaming test `test_process_stream_chunks_carries_cache_usage_to_meta` still passes, and `test_non_streaming_tool_result_emits_tool_call_update` now also asserts a usage-free stream reports no cache usage.

    Note for whoever runs the acceptance check: the fleet log line will read `reuse="warm prompt cache" cache_read_input_tokens=Some(N)` only for a process running this build. The prime stage is untouched, as the card requires.

    Two things found and NOT acted on:
    - The stub had to answer the init turn with its own `result` line. Without it `consume_remaining_init_response` burns its full wait before `new_session` returns (35s in the first draft, 0.5s after). Worth knowing for any future scripted-CLI test.
    - `crates/swissarmyhammer-mcp-proxy/.kanban` is a stray board directory, committed back in 375d20b16. Pre-existing and out of scope here.
  timestamp: 2026-08-08T09:47:36.440354+00:00
- actor: claude-code
  id: 01kzgcaz2p7m1w12b8cbrgfnax
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/claude-agent/src/protocol_translator.rs, crates/claude-agent/src/agent_prompt_handling.rs, crates/claude-agent/src/agent.rs, crates/claude-agent/src/lib.rs, crates/claude-agent/src/claude.rs, crates/claude-agent/src/test_support.rs, crates/swissarmyhammer-validators/src/validators/pool.rs. Two production-path tests watched RED then GREEN against a scripted `claude` on PATH. `cargo nextest run --workspace`: 13791 passed, 0 failed, 0 skipped. `cargo fmt` applied. `cargo clippy --workspace --all-targets -- -D warnings`: clean. Known unrelated flake `collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes` (^7mk3pnp) passed in the full run; untouched.
    - next: /review
  timestamp: 2026-08-08T09:47:46.646063+00:00
position_column: doing
position_ordinal: '8480'
title: 'review fleet: warm prefix reuse never engages — every fork runs cold'
---
DIAGNOSED 2026-08-07 from ../swissarmyhammer-main/.sah/mcp.7341.log. The forks ARE warm. The telemetry is broken. This is a log-lies defect, not a performance defect.

Evidence:
- The prime turn (15:14:05) reported `cache_read: 0, cache_creation: 68324` — the shared prefix wrote to the Anthropic prompt cache.
- Every one of the nine forked task turns reported `cache_read_input_tokens: Some(68324)` on the translator's final chunk ("Sending final chunk", claude.rs:1177). The full primed prefix came from the warm cache on every fork. The fork mechanism (`claude --resume <prime> --fork-session`) works.
- The fleet reuse line for the same tasks says `reuse="cold (no reuse)" cache_read_input_tokens=None`. The usage never reached `SessionTurn`.

Root cause (confirmed in source):
- The pool reads `cache_usage` from `PromptResponse._meta` (`validators/pool.rs` ~900-916).
- The real claude-agent never puts it there. `handle_streaming_prompt`'s response builder (`claude-agent/src/agent_prompt_handling.rs` ~618-627) inserts `streaming`, `session_id`, `output_tokens` — no `cache_usage`. The value stops at the internal final chunk (`claude.rs:1174`).
- The pool test `test_pool_turn_propagates_cache_usage_from_response` passes because its MOCK attaches the `_meta.cache_usage` key the real agent never sends. Mock drift hid the gap (see the real-path-tests rule).

Fix:
1. In claude-agent, keep the final StreamResult's `cache_usage` and insert `cache_usage: CacheUsage::to_meta_json(...)` into the PromptResponse `_meta`, for streaming and non-streaming prompts. `to_meta_json` exists; the pool already parses that exact shape.
2. A turn with several result messages keeps the last non-empty usage. Any read > 0 classifies warm; more precision is not needed.
3. The verify path reads the same `SessionTurn`; one fix covers both "degraded" log sites.

Tests:
- claude-agent production-path test: a scripted claude process emits a result line with a `usage` object (the shape at `protocol_translator.rs:1464`); assert the returned PromptResponse carries `_meta.cache_usage.cache_read_input_tokens` equal to the emitted value. This is the test the mock hid.
- Keep the existing pool test — the pool side is already proven.
- Acceptance on a real run: the fleet log line reads `reuse="warm prompt cache" cache_read_input_tokens=Some(N)`, with N near the prime's cache_creation count.

Scope correction: the first version of this card said cold forks were the dominant run cost. They are not — the API served the prefix warm on every fork. The cost of this defect is misleading telemetry and the wrong conclusions built on it. Do NOT remove the prime stage. It works.

#tool-validators