---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9680
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