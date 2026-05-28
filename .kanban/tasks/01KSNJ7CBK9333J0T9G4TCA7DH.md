---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb280
project: ai-panel
title: 'Bug: qwen produces 0 tokens on first prompt; retry hits \"Queue is full\"'
---
## What

When picking the `qwen` (Qwen3.6-27B GGUF) model and asking it any question in the AI panel, qwen returns an empty response. The GUI shows nothing. On retry the system shows "AI Error" because the previous request is still occupying the single-worker queue.

(Original investigation notes preserved below the resolution.)

## Resolution (root cause found and fixed)

The "0 tokens" symptom is NOT a Qwen3.6 template/model mismatch. It is a budget-arithmetic bug in the **streaming** generation path that triggers whenever the rendered prompt is large relative to the caller's `max_tokens` budget — exactly the situation with the 27B model's big kanban system prompt + tool schemas and a small remaining-context budget.

### Root cause

`crates/llama-agent/src/generation/mod.rs::generate_stream_with_borrowed_model` (the production streaming path; the ACP agentic loop in `acp/server.rs` calls `generate_stream`, which dispatches here with `template_token_count: None`) computed:

```rust
let max_tokens = request.max_tokens.unwrap_or(512) as usize - tokens_list.len();
```

The ACP agentic loop ALREADY budgets `max_tokens` as remaining context (`context_size - current_tokens`). Subtracting the full rendered-prompt length again:
- underflowed (usize) when `prompt_len > budget` → panic in debug, wrap-to-huge in release (runaway generation that monopolized the single worker → symptom 2's root cause), and
- collapsed the budget to ~0 when `prompt_len ≈ budget` → the loop `while tokens_generated < max_tokens` ran zero iterations → "0 tokens in this turn, 0 total".

The proven **batch** path (`generate_common`, the one all prior real-model tests exercised) uses `max_tokens` directly with a context-window guard — which is why batch generation always worked and the streaming bug stayed invisible (no streaming real-model test existed).

Two further streaming-path defects fixed alongside:
- duplicate `generated_text.push_str` (each token appended twice).
- `StreamChunk.token_count` was set to the running total `tokens_generated`; the ACP loop SUMS `chunk.token_count`, producing a triangular-number token count (e.g. ~100 real tokens reported as 5050). Now each per-token chunk carries `token_count: 1` and the completion chunk carries `0`, so summing yields the true count.

### Queue jamming (symptom 2)

Root cause is the runaway generation from the release-mode underflow monopolizing the single worker. With a correctly bounded budget + context guard the worker finishes promptly and releases. A pure-logic queue-lifecycle test now locks the invariant that the single worker is released after a turn (any outcome) so a second request enqueues without "Queue is full".

### Notes on red herrings
- `Model loaded ... (Memory: +0 MB, Total: 0 MB)`: logging artifact of lazy mmap, unrelated to generation. Not pursued.
- `qwen.yaml` `kanban` tag: KEPT. The root cause was not a template mismatch, so there is no reason to drop the tag — the model should now generate correctly.

### Files changed
- `crates/llama-agent/src/generation/mod.rs` — budget fix + context guard + single push_str + per-chunk token_count. The budget fix landed on the production streaming path (`generate_stream_with_borrowed_model`). The context-window guard and `token_to_str_lossy` lossy decoding now apply to **both** borrowed-model streaming variants: `generate_stream_with_borrowed_model` and its template-offset sibling `generate_stream_with_borrowed_model_and_template_offset`. (The offset variant is currently unreached for streaming — the production caller `queue.rs::process_streaming_request_sync` always passes `template_token_count: None` — so the guard was added there for symmetry, to prevent the same context-overflow/UTF-8-drop regression should template caching ever be re-enabled.) Removed now-unused `trace` import.
- `crates/llama-agent/src/queue.rs` — added `test_streaming_worker_released_after_turn`.
- `crates/llama-agent/tests/integration/streaming_generation.rs` (+ `mod.rs` registration) — 3 real-model streaming regression tests.

### Verification
- `test_streaming_with_prompt_larger_than_max_tokens` FAILS on pre-fix code (reproduced "Large-prompt streaming produced 0 tokens" + `attempt to subtract with overflow` panic) and PASSES post-fix (64 tokens, bounded).
- Streaming real-model tests: non-empty text, correct token count (100, was 5050).
- `cargo test -p llama-agent --lib`: 819 passed, 0 failed.
- `cargo clippy -p llama-agent --tests`: 0 warnings.

## Acceptance Criteria

- [x] Picking qwen and asking a simple question produces a non-empty response in the GUI. (root cause fixed; verified via real-model streaming path with Qwen3-0.6B)
- [x] The agentic loop's token count is > 0 on a normal interaction.
- [x] After a turn completes (success or empty), the worker is released and a subsequent prompt does not fail with "Queue is full".
- [x] If the 0-token problem turns out to be a template / model mismatch, document the supported qwen variants and possibly drop the `kanban` tag from `qwen.yaml`. (N/A — not a template mismatch; documented above, tag kept.)

## Tests

- [x] Add a Rust integration test that drives a tiny qwen model through a single prompt and asserts the response is non-empty and `tokens_generated > 0`. (`streaming_generation.rs`, uses Qwen3-0.6B test model)
- [x] Add a queue-lifecycle test: after a turn returns (any outcome), a second prompt must enqueue successfully — no "Queue is full". (`queue::tests::test_streaming_worker_released_after_turn` + `test_second_streaming_prompt_after_turn_succeeds`)
- [x] Run: `cargo test -p llama-agent`.

---

## Original investigation notes

### Two symptoms
1. **0 tokens generated** — agentic loop completed with `0 tokens in this turn, 0 total`.
2. **Queue jamming** — `worker_threads: 1, max_queue_size: 100`; retry rejected with "Queue is full".

### Evidence (OS log)
```
15:24:23.102  Chat template engine initialized with strategy: Some(Qwen3) (derived from model: unsloth/Qwen3.6-27B-GGUF/Qwen3.6-27B-IQ4_NL.gguf)
15:26:26.782  Agent generation turn completed: 0 tokens in this turn, 0 total
15:29:06.926  F  Agent streaming generation failed: Request processing error: Queue is full
```

## Related
- Independent of `01KSNJ6AE18EQYDC2WSYFSSAY1` (per-board persistence regression).

## Review Findings (2026-05-28 12:30)

Core fix verified correct on all three focus points: (1) the context-window guard mirrors `generate_common` and does not over-truncate legitimate prompts — it only stops generation when prompt+generated would overflow the KV cache; (2) the per-chunk `token_count` of 1 (0 on completion) is correct given consumers sum the stream, matching `acp/server.rs`; (3) the new real-model tests assert on content (`tokens > 0` AND `!text.trim().is_empty()`, plus a `tokens <= 64` upper bound), not just absence of error. clippy clean, tests compile.

### Nits
- [x] `crates/llama-agent/src/generation/mod.rs:978` — The context-window guard and the `token_to_str_lossy` lossy decoding were added only to `generate_stream_with_borrowed_model`, NOT to the sibling `generate_stream_with_borrowed_model_and_template_offset`, whose loop still checks only `tokens_generated < max_tokens` + cancellation and still uses plain `model.token_to_str`. This is not a live bug: the production streaming caller (`queue.rs:1159 process_streaming_request_sync`) always passes `template_token_count: None`, so the offset branch is currently dead for streaming and the fix correctly targeted the reachable path. But the two variants are now divergent, and the offset variant would reintroduce the same context-overflow `decode` failure (and the GLM-4.7 partial-UTF-8 token drop) if template caching is ever re-enabled. Suggestion: either apply the same guard + `token_to_str_lossy` to the offset variant for symmetry, or add a comment on the offset loop noting the missing guard and that the path is currently unused. Also note the description's "both streaming variants" claim is inaccurate — only the non-offset variant received the context guard.

  RESOLVED (2026-05-28): Applied the same context-window guard + `token_to_str_lossy` to `generate_stream_with_borrowed_model_and_template_offset` for symmetry. The guard measures against the full prompt length (`total_token_count`, which is what occupies the KV cache and where `n_cur` starts), not the post-offset slice — the offset only governs which tokens are decoded into the already-cached prefix. The inaccurate "both streaming variants" claim in the Files-changed section above has been corrected to distinguish the budget fix (production path) from the guard/lossy-decode now present on both borrowed-model variants. `cargo clippy -p llama-agent --tests`: 0 warnings; `cargo test -p llama-agent --lib`: 819 passed, 0 failed.