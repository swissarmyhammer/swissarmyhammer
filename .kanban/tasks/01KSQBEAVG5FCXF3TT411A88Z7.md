---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
- 01KSQNC9P6F5N7SHYVGXW5JZ6G
position_column: todo
position_ordinal: '8980'
project: llama-coverage
title: Cover generation core (mod.rs + generator.rs) via scripted model — budget, streaming, chunk accounting
---
## RESCOPE (added after keystone review 01KSQBDM9M4RJJYGQDTZYJA107)

The keystone's `ScriptedModel` implements `TextGenerator` at the WHOLE-call level — it does NOT exercise the real internal decode loops (`generation/mod.rs::generate_stream_with_borrowed_model*`, `generate_common`, `token_to_str_lossy`, the context guard, and the stale `generator.rs::generate_stream_with_context`). Those bind `LlamaContext`/`LlamaBatch`/`LlamaSampler` and are unreachable through `ScriptedModel`. So the original ">95% of generation via ScriptedModel" goal is NOT achievable as written.

Choose per-region:
- **Budget/chunk arithmetic that can be extracted into pure functions** — extract it (e.g. a `remaining_budget(max_tokens, prompt_len)` and a chunk-count helper) and unit-test those exhaustively with no model. This is the highest-value move and would have caught the original bug directly.
- **The raw decode loops that genuinely bind llama.cpp** — cover with the small real model (qwen-0.6B), mirroring `tests/integration/streaming_generation.rs`. Do NOT pretend ScriptedModel covers them.
- **The stale `generator.rs` duplicate** — do NOT add coverage to it here; it's handled by dedup card `01KSQNC9P6F5N7SHYVGXW5JZ6G` (likely deleted). Depend on that card so this one targets only the canonical loop.

## What (original)

Drive the canonical generation engine (`crates/llama-agent/src/generation/mod.rs`, and `generation/generator.rs` only if the dedup card keeps it) to near-complete behavioral coverage. This is where the 0-token bug lived; lock the whole path down.

## Cover, at minimum

- **Budget arithmetic** (prefer extracted pure fn): prompt_len < budget, ≈ budget, > budget. The ≈ and > cases must NOT yield 0 or underflow.
- **Both streaming variants** in mod.rs — `generate_stream_with_borrowed_model` AND `..._and_template_offset`.
- **Batch path** — `generate_common`, parity with streaming on budget + stop.
- **Chunk accounting** — `StreamChunk.token_count` per-chunk delta; sum across a stream equals the real total (triangular-number guard).
- **Context-window guard** — boundary: exactly at limit, one under, one over.
- **token_to_str_lossy** — multi-byte UTF-8 split across two decode steps reassembles (no dropped bytes).
- **Completion reasons** — EOS / max_tokens / context-window-full each produce the correct finish reason.

## Acceptance Criteria

- [ ] Budget/chunk arithmetic covered (via extracted pure fns where feasible, real small model otherwise) — the three budget regimes pinned.
- [ ] Both mod.rs streaming variants + batch path covered.
- [ ] Coverage of the canonical generation path rises to the threshold from the gap map (target >95% for the pure-extractable parts; real-model smoke for the FFI loop). Justify exclusions.
- [ ] No reliance on ScriptedModel to cover the raw decode loop — that's explicitly out of its reach.

## Tests

- [ ] Pure-fn unit tests for extracted arithmetic; real-small-model tests for the decode loop in `tests/integration/`.
- [ ] Run: `cargo test -p llama-agent generation` and a scoped `cargo llvm-cov --package llama-agent` to confirm the delta vs the 78.01% baseline.

## Workflow

- Use `/tdd`. Consult the gap map in card `01KSQBCTMV4K3ATFZ5RFQ0FJBB`.
- Depends on the dedup card `01KSQNC9P6F5N7SHYVGXW5JZ6G` so there's a single canonical loop to target.