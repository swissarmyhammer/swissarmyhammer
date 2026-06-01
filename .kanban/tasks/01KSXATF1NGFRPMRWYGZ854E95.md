---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd080
title: 'MTP perf: persist draft context state across turns (eliminate doubled prefill on agentic re-entry)'
---
Telemetry from `cargo run --example mtp_smoke` shows the MTP algorithm wins on generation: ~3.4 accepted drafts per round, ~4 tokens emitted per target forward pass, ~50–58 tok/s on Qwen3.6-35B-A3B-MTP-GGUF. Commit 5fa3ad28e.

BUT for the kanban hot path — 28k-token prompt (system + 10 MCP tool schemas + history) + short 30–100-token replies — the per-chunk `sync_capture` decodes the *entire* prompt on the **draft** context as well as the target. That doubles prefill cost (~25s extra on the 35B), and the generation-side speedup can't pay it back for short replies. Net result: MTP is currently SLOWER than standard streaming for this case, which is what the user observed ("a lot slower than opus", "still doing SOMETHING wrong performance wise").

## Fix
Save/restore the **draft** context's per-seq state alongside the target's, in the same `SessionStateStore` lifecycle that already exists for the target.

- `queue.rs::save_session_state` already snapshots the target via `copy_state_data`. Extend it to also snapshot the draft via `state_seq_get_data(seq_id=0)` and store both in `SessionStateStore` keyed by session id (struct already holds `state_bytes` + `prompt_tokens` — add `draft_state_bytes`).
- `prepare_streaming_kv_cache` / its sibling for the draft: on turn 2+, restore the draft state, then only `sync_capture` the **new** tokens (the tool-result + new gen-prompt suffix — typically <100 tokens, one batch). No more re-decoding 28k tokens on the draft.
- Existing LCP / `n_rs_seq` rollback logic on the target already handles partial-prefix divergence; same `Ok(false)` fallback applies to the draft restore.
- Verify with the two-turn `mtp_smoke` already added (commit 3203e2827) using a long prompt: turn 1 pays the one-time prefill (cold), turn 2 should be ~target-only generation cost.

Acceptance:
- Two-turn smoke with a synthetic 28k-token prompt: turn 2's elapsed time drops to roughly target-prefill + target-generation; draft prefill should NOT re-decode the cached prefix.
- Telemetry (already logged per turn) still shows healthy acceptance (~3+ tok/pass) on turn 2.

Files: crates/llama-agent/src/queue.rs (SessionStateStore + save_session_state + prepare_streaming_kv_cache), crates/llama-agent/src/generation/mtp/streaming.rs (skip prefill sync_capture on the prior-cached range when a draft snapshot is restored).