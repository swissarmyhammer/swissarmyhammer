---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffce80
title: Implement MTP draft-mtp speculative decoding in llama-agent (consumer side)
---
## SHIPPED (locally — gated on fork push)
qwen runs draft-mtp speculative decoding end-to-end via the streaming path. Auto-detected via `LlamaModel::has_mtp()` — no config; non-MTP path unchanged.

### Final piece (the actual blocker): `n_rs_seq` context param
Commit **3e959733c**. The hybrid attention+gated-delta-net target (Qwen3.5/3.6) wasn't an algorithmic dead-end — llama.cpp's `llama_memory_recurrent::seq_rm` *does* support partial rollback via a per-token state-snapshot ring of size `cparams.n_rs_seq` (`src/llama-memory-recurrent.cpp:181-189`). The default is 0, which makes every partial seq_rm silently return false (`max_pos` stays high; next batch trips M-RoPE). Setting `n_rs_seq=8` on context creation enables the rollback window. llama.cpp clamps to 0 on non-recurrent arches automatically, so it's free elsewhere; recurrent state is tiny, so the window costs nothing meaningful. The fork's own `mtp_matches_greedy_and_accepts` test also fails by default for the same reason — the algorithm is correct, the host just has to opt in to the rollback window.

### Consumer commits (local, gated on fork push)
- 605e2879c qwen→MTP GGUF + pure helpers (30 tests)
- 1f9af7a88 MtpSession port (sync_capture/draft/verify/accept)
- 77d30d9ef MtpParams serde
- c870428f1 streaming integration in queue.rs
- e4e1ce0ff small MTP test model builtin + draft snapshot/restore + mtp_streaming.rs scaffold
- 5ee3a18ac chunking-bug fix in prefill + mtp_smoke example
- **3e959733c n_rs_seq fix** — the keystone

### Fork edits (uncommitted in ../llama-cpp-rs, must go with fork push)
- src/llama-ext.h decl + src/llama-model.cpp impl + wrapper_ext.h shim + build.rs allowlist for `llama_model_nextn_predict_layers`.
- Rust `LlamaModel::has_mtp()` / `nextn_predict_layers()`.
- Rust `LlamaContextParams::with_n_rs_seq()` / `n_rs_seq()`.

### Verified on Metal via cargo run --example mtp_smoke
- Qwen3.5-0.8B-MTP-GGUF — 64 tokens / 767 ms / MaxTokens, coherent `<think>` text.
- Qwen3.6-35B-A3B-MTP-GGUF — 64 tokens / 1.05 s / MaxTokens, coherent `<think>` text.

### Nice-to-haves (NOT blocking)
- Formal greedy-MTP == greedy-non-MTP token-for-token equivalence test (the smoke proves the loop works; the fork's correctness.rs is the algorithm authority).
- Revert MtpSession::sync_capture's snapshot/restore hack to a simple partial-clear matching the fork reference (now redundant with n_rs_seq>0; harmless to keep).

### Push order
1. push fork (incl. nextn_predict_layers + n_rs_seq builder + the rest of the MTP bindings).
2. flip workspace Cargo.toml off path deps to new git rev.
3. push consumer commits 605e2879c..3e959733c.