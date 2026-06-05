---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc980
title: Review of d4a69cbe8 (streaming KV-cache reuse, llama-agent)
---
Scope: commit d4a69cbe8 vs parent e0d93d061 — crates/llama-agent (queue.rs, acp/server.rs, tests/integration/streaming_generation.rs)

## RESOLUTION (2026-05-29, follow-up commit pending)
- [x] BLOCKER (partial KV save on disconnect/cancel) — FIXED. `queue.rs` now gates the save through `should_persist_stream_state(result_ok, cancelled, sender_closed)` = save only on clean completion with the receiver still attached. Disconnect (`sender.is_closed()`) and cancellation (`cancellation_token.is_cancelled()`) skip the save → correct cold start next turn instead of corruption. +2 unit tests.
- [x] WARNING #2 (final assistant turn not persisted) — FIXED. `acp/server.rs` now adds the final assistant message (raw generated_text) before the no-tool-calls `break`, completing history and preserving cross-prompt cache validity.
- [x] WARNING #3 (length-vs-content prefix guard) — DOCUMENTED in `prepare_streaming_kv_cache` (prefix assumption) + robust fix tracked in card 01KSSSPN67B23A0B8TRPCRNC34 (content fingerprint / longest-common-prefix).
- [~] WARNING memory/eviction (full-state copy + non-LRU, count-based eviction) — tracked in 01KSSSPYEG33YZA2WJ8N9Y69V2 (true LRU + byte budget). Pre-existing in batch path.
- [~] WARNING concurrency (no per-session lock; safe only at worker_threads=1) — tracked in 01KSSSQ6EP42C2TCHJWNY2JFNH.
- [~] WARNING streaming/batch gate divergence — acceptable: a session is driven by exactly one path (ACP=streaming, validator/title=batch); noted in 01KSSSQ6EP42C2TCHJWNY2JFNH.
- [~] WARNING test vacuous-pass + NITS (dedupe offset fns, double tokenization) — tracked in 01KSSSQG9AYKKK8CPG4X61RA8E.

Verification after fixes: clippy clean; 10 queue free-fn unit tests pass; 27 real-model integration tests pass (acp_agentic_loop incl. cancel, tool_use_multi_turn, streaming_generation incl. KV-reuse, acp_read/write_file).

---
(original findings below)

### Blockers
- [x] `crates/llama-agent/src/queue.rs:1283-1290` — Partial KV state saved after early stream disconnect. FIXED via should_persist_stream_state gate.

### Warnings
- [x] `crates/llama-agent/src/acp/server.rs` — final assistant turn not persisted. FIXED.
- [x] `streaming_offset_decision` length-not-content guard — DOCUMENTED + card 01KSSSPN67B23A0B8TRPCRNC34.
- [~] save_session_state full-state copy / byte ceiling — card 01KSSSPYEG33YZA2WJ8N9Y69V2.
- [~] evict_oldest_session_states not real LRU — card 01KSSSPYEG33YZA2WJ8N9Y69V2.
- [~] streaming vs batch reuse-eligibility gate divergence — card 01KSSSQ6EP42C2TCHJWNY2JFNH.
- [~] SessionStateCache no per-session lock (single-worker assumption) — card 01KSSSQ6EP42C2TCHJWNY2JFNH.
- [~] KV-reuse test can pass vacuously under shared-process cargo test — card 01KSSSQG9AYKKK8CPG4X61RA8E.

### Nits
- [~] double tokenization — card 01KSSSQG9AYKKK8CPG4X61RA8E.
- [~] streaming_offset_decision duplicates compute_template_token_count — card 01KSSSQG9AYKKK8CPG4X61RA8E.

#review #llama-agent