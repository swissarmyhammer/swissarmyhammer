---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc780
project: ai-panel
title: 'Best-performance llama context params on Metal: flash attention + quantized KV cache'
---
## DONE (2026-05-29)

Best-performance context params now applied to every llama model in `ModelManager::create_context` (`crates/llama-agent/src/model.rs`):
- **Flash attention ENABLED** (`with_flash_attention_policy(LLAMA_FLASH_ATTN_TYPE_ENABLED)`) — big win on Metal and required for a quantized V cache.
- **Quantized KV cache Q8_0** for both K and V (`with_type_k`/`with_type_v(KvCacheType::Q8_0)`) — near-lossless, ~half the F16 KV memory.

### No env vars (per user)
An earlier cut used `LLAMA_KV_CACHE_TYPE` / `LLAMA_FLASH_ATTN` env overrides; removed. These are now baked best-Metal defaults, not env/user knobs — the model config drives everything else about the load. (Per-model YAML override would require threading two fields across swissarmyhammer-config `LlmModelConfig` → swissarmyhammer-agent conversion → `llama_agent::types::ModelConfig` and updating ~71 `ModelConfig {…}` literals; deferred unless wanted.)

### Plumbing
- Added `llama-cpp-sys-2` (same git as llama-cpp-2, Cargo unifies to one build) to name `LLAMA_FLASH_ATTN_TYPE_ENABLED`.
- Imported `KvCacheType`.

### Verification (proven, not guessed)
- `metal_gpu` test now creates a context inside the stderr-capture window (FA/KV are set at context creation, not model load) and asserts on the real llama.cpp log lines:
  - `llama_context: flash_attn = enabled`
  - `llama_kv_cache: … k (q8_0): … mib, v (q8_0): … mib`
  plus the existing all-layers-offloaded-to-GPU assertion. Passes.
- Real-model `acp_single_turn` still generates tokens + clean stream with the new context params — no quality regression.
- fmt clean, clippy 0, `cargo check -p kanban-app` green (sys dep unified, app builds).
- Skip message uses `tracing::warn!` (not eprintln), per convention.

### Acceptance criteria
- [x] Generation context enables flash attention + quantized (Q8_0) KV cache.
- [~] Escape hatch: dropped per "no env vars". Defaults are the best-Metal values; per-model YAML override is the deferred follow-up.
- [x] macOS Metal test asserts FA enabled AND KV cache type is q8_0 (both sides), from the load logs.
- [x] All layers still offload to Metal; real-model tests still pass.
- [x] No quality regression in real-model generation.