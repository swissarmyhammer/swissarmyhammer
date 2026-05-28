---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
- 01KSQNC9P6F5N7SHYVGXW5JZ6G
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbc80
project: llama-coverage
title: Cover generation core (mod.rs + generator.rs) via scripted model — budget, streaming, chunk accounting
---
## DONE (2026-05-28)

Resolved per the rescope. Highest-value move taken: extracted the budget/context arithmetic — the exact home of the 0-token bug — into a pure, model-free `generation/budget.rs` module and routed all production call sites through it.

### What landed
- New `crates/llama-agent/src/generation/budget.rs` with three pure fns + 11 exhaustive unit tests (no model):
  - `generation_budget(Option<u32>)` — the budget is the caller's value verbatim, NOT reduced by prompt length (the double-subtraction bug). Pins None→512, Some(0)→0, Some(MAX) regimes.
  - `reached_context_limit(prompt, generated, ctx_size)` — saturating; boundary one-under/at/over + degenerate ctx_size=0 (no panic). Replaces the inconsistent `ctx_size - 1` (panic risk) in `generate_common` and `saturating_sub(1)` in the streaming paths with one shared, tested predicate.
  - `template_offset_exhausted(offset, total)` — equal/exceeds without underflow.
- Refactored all 4 budget sites + 3 context-guard sites + 2 template-offset sites in `generation/mod.rs` to call these. Behavior preserved (verified by full suite).
- Rewrote the weak inline-arithmetic `template_offset_tests` in `generation/tests.rs` to exercise the real `budget::` predicate instead of re-deriving the math.
- Added a real-model integration test `test_streaming_completion_reason_and_chunk_accounting` (streaming_generation.rs) pinning: exactly one terminal `is_complete` chunk carrying a `finish_reason`; per-token chunks carry `token_count == 1`, terminal carries 0; `sum(token_count) == text_chunks` (no double-count). Passed: 64 chunks, summed=64, reason=MaxTokens.

### Coverage (crate-scoped, same methodology as baseline card)
- `cargo llvm-cov report --package llama-agent --lcov` → `scripts/llama_agent_gap_report.py`
- `budget.rs`: 100% (region/line/function). `config.rs`/`error.rs`: 100%.
- `generation/mod.rs`: 39.43% — the remainder is the four near-identical FFI decode loops (LlamaContext/LlamaBatch/LlamaSampler) whose happy paths are hit by real-model smoke (streaming_generation.rs + acp_agentic_loop) but whose llama.cpp error branches are not unit-reachable. This is the justified [MODEL] exclusion from the gap map.
- **Crate total: 84.29% line (20344/24136), up from 78.01% baseline.**

### Acceptance criteria
- [x] Budget/chunk arithmetic covered via extracted pure fns — three budget regimes pinned.
- [x] Both mod.rs streaming variants + batch path covered (real-model smoke).
- [x] Pure-extractable parts at 100%; FFI loop has real-model smoke; exclusions justified above.
- [x] No reliance on ScriptedModel for the raw decode loop.