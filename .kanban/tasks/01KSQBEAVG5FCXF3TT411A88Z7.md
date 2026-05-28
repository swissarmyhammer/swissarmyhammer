---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: todo
position_ordinal: '8980'
project: llama-coverage
title: Cover generation core (mod.rs + generator.rs) via scripted model — budget, streaming, chunk accounting
---
## What

Drive the generation engine (`crates/llama-agent/src/generation/mod.rs`, `generation/generator.rs`) to near-complete behavioral coverage using the `ScriptedModel` from the harness card. This is where the 0-token bug lived; lock the whole path down.

## Cover, at minimum

- **Budget arithmetic** — the exact bug: prompt_len < budget, prompt_len ≈ budget, prompt_len > budget. Assert the right number of tokens generates in each case; the ≈ and > cases must NOT yield 0 or underflow.
- **Both streaming variants** — `generate_stream_with_borrowed_model` AND `generate_stream_with_borrowed_model_and_template_offset` (the offset variant currently dead for streaming but must be covered so a re-enable can't regress).
- **Batch path** — `generate_common`, parity with streaming on budget + stop behavior.
- **Chunk accounting** — `StreamChunk.token_count` per-chunk delta; sum across a stream equals the real total (guard against the triangular-number regression).
- **Context-window guard** — `prompt_tokens + generated >= context_size - 1` boundary: exactly at the limit, one under, one over.
- **token_to_str_lossy** — a multi-byte UTF-8 sequence split across two decode steps reassembles correctly (no dropped bytes).
- **Completion reasons** — EOS hit, max_tokens hit, context-window-full — each produces the correct completion/finish reason.

## Acceptance Criteria

- [ ] Every public generation entry point is exercised by at least one scripted-model test.
- [ ] The three budget regimes (under/≈/over) are each pinned with explicit assertions.
- [ ] Coverage of `generation/mod.rs` + `generation/generator.rs` rises to the threshold agreed in the gap-map card (target: >95% region coverage, justify any deliberate exclusions).
- [ ] All new tests use `ScriptedModel` — fast, deterministic, no real-model download.

## Tests

- [ ] Add to `crates/llama-agent/tests/integration/` or a `generation/tests.rs` module: budget-regime tests, chunk-accounting test, context-guard boundary tests, lossy-decode split test, completion-reason tests.
- [ ] Run: `cargo test -p llama-agent generation` and a scoped `cargo llvm-cov --package llama-agent -- generation` to confirm the coverage delta.

## Workflow

- Use `/tdd`. Build on the scripted-model keystone — do not reach for a real model for logic paths.
- Consult the gap map from card `01KSQBCTMV4K3ATFZ5RFQ0FJBB` for the specific uncovered regions.