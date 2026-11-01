---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc580
project: ai-panel
title: Verify llama-agent uses Metal GPU on macOS (small-model test)
---
## DONE (2026-05-28) — Metal CONFIRMED in use

The user's CPU suspicion was wrong, now proven not guessed. New macOS-only test `crates/llama-agent/tests/integration/metal_gpu.rs::qwen_small_model_loads_on_metal_gpu` loads `qwen-0.6b-test` and the captured llama.cpp logs show:
- `ggml_metal_device_init: GPU name: MTL0 (Apple M5 Max)`
- `load_tensors: offloaded 29/29 layers to GPU`

So the model runs fully on the Metal GPU. The GUI slowness on the 27B was NOT CPU — it was the 28k-token prompt (10 kanban MCP tool schemas rendered into a 108k-char prompt) on a 27B with a 262k context window (prefill cost), plus Qwen3 thinking mode.

### How the test works
- Builds a `ModelManager` with `debug=true` (un-suppresses llama.cpp native logging).
- Those ggml/llama logs go straight to the C `stderr` fd — NOT through the `tracing` bridge (an in-process tracing subscriber captured nothing, which is why the first cut failed even though Metal was clearly working). So the test redirects fd 2 to a temp file across the load (`StderrCapture` RAII via `libc::dup`/`dup2`) and reads it back.
- Hard assertion: parse `offloaded N/N layers to GPU` and require N==total>0 (every layer on the GPU — on macOS the only GPU backend is Metal, so this == running on Metal). This line fires on every load. Also asserts "metal" appears (the ggml device-init line; reliable under nextest's process-per-test).
- No timing assertion.
- Added `libc` dev-dep.

### Verification
- `cargo test -p llama-agent --test agent_tests integration::metal_gpu` → ok.
- `cargo nextest run -E 'test(integration::metal_gpu)'` → 1 passed (2.2s). clippy clean.

### Why it found nothing to fix
Metal was already correctly enabled: `llama-cpp-2` built with `features=["metal","sampler"]`, and `default_model_params` defaults `n_gpu_layers` to `i32::MAX` (offload all) when `LLAMA_N_GPU_LAYERS` is unset. The test now guards that this stays true.

### Acceptance criteria
- [x] macOS test loads qwen-0.6b-test and asserts Metal active with all layers offloaded (29/29).
- [x] No timing-based assertion (parses the offload-count log line directly).
- [x] Investigated the "is it CPU?" concern — it is NOT; Metal is in use, nothing to fix there. (Real slowness cause = prompt size, tracked separately.)