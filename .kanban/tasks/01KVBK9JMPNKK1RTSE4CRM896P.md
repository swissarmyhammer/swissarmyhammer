---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: kv-prefix-reuse
title: 'Real-model integration test: sibling review turns reuse the pinned prefix (no full reprocess)'
---
## What
Prove end-to-end on a REAL recurrent model that primed-prefix + sibling file turns reuse the pinned prefix donor with rollback 0: the `KV trim to common prefix returned false … invalidating cache` WARN (`crates/llama-agent/src/queue.rs:2646`) never fires, and the `streaming reusing N cached tokens` INFO (`queue.rs:2663`) does fire with N == prefix length.

IMPORTANT (corrected from initial plan): the scripted fake model CANNOT exercise this — `prepare_streaming_kv_cache` (`queue.rs:2517`) is a private fn that takes a real `&mut LlamaContext` + `&LlamaModel` and the trim (`clear_kv_cache_seq`) is real FFI behavior the scripted `TextGenerator` double (operates above FFI) does not provide. There is no seam to inject a fake context, and `StreamingKvPrep` is private. So this must be a real-model, log-scraping test.

Model it on the existing `crates/llama-agent/tests/integration/session_fork_real_model.rs` (`forked_sessions_reuse_full_parent_prefix_without_rollback`), gated the same way as the other real-model GPU tests (the self-hosted CI runner has a Metal GPU — see memory `ci-runner-has-gpu`; do NOT force CPU). Use the qwen hybrid model.

- Prime a validator prefix (pinned), then run ≥2 sibling turns whose prompts share the prefix and diverge with tails > 64 tokens.
- Assert the queue log shows `streaming reusing <prefix_len> cached tokens` on each sibling turn and ZERO `KV trim to common prefix returned false` WARN lines.
- Also observe whether `skipping MTP this turn` (the draft-KV fallback, `queue.rs:2762`) fires on sibling turns, so the MTP-draft residual (tracked by the MTP draft-KV task) is measured, not silently shipped.

The pure-selector assertion (prime chosen over sibling under max_rollback=64) is already covered by the keystone's unit tests — this task is the integration proof that the real FFI trim succeeds.

## Acceptance Criteria
- [ ] A gated real-model integration test in `crates/llama-agent/tests/integration/` runs prime + ≥2 sibling turns on the qwen hybrid model through the live queue.
- [ ] Asserts the `streaming reusing N cached tokens` INFO fires with N == prefix length on every sibling turn.
- [ ] Asserts ZERO `KV trim to common prefix returned false` WARN lines across the run.
- [ ] Records (asserts or logs) whether `skipping MTP this turn` fires, feeding the MTP-draft task.

## Tests
- [ ] New gated test e.g. `crates/llama-agent/tests/integration/kv_prefix_reuse_recurrent.rs` (same GPU gate as `session_fork_real_model.rs`).
- [ ] Runs green on the GPU CI runner; full `cargo test -p llama-agent` green (test skips cleanly where the model/GPU is unavailable, per existing real-model test convention).

## Depends on (prose — kanban depends_on edges are currently dropped by a known bug): keystone 01KVBK83218VM915ZTVZCKZ9VA, model-wiring 01KVBK8MTE2CJ7D4PS4X3181NX, tool-order 01KVBK8Z7WKZ96APERDP8DTHQ6, and benefits from the MTP-draft task.

## Workflow
- Use `/tdd` — run it against pre-fix behavior to see the WARN/full-reprocess (RED), then GREEN after the selector + wiring land.