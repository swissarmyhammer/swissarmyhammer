---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
project: llama-coverage
title: Scripted fake model behind an inference trait (the deterministic test keystone)
---
## What

The only part of llama-agent that genuinely needs a real model is the FFI decode (feed tokens → llama.cpp → logits). Everything above it — budget arithmetic, the streaming loop, stop conditions, chunk accounting, the queue worker, the ACP agentic turn loop — is deterministic logic that should be driven without a GPU or weights.

Build the seam that makes that possible: an **inference trait** that abstracts the per-step "given the current token context, produce the next token (or EOS)" operation, plus a **scripted fake model** implementing it that returns predetermined tokens. This is the keystone the model-dependent coverage cards depend on.

## Investigate first

- `crates/llama-agent/src/model.rs` (1138 lines) — find where the real model exposes its decode/sample step. Identify the smallest trait boundary that the generation paths (`generation/mod.rs`, `generation/generator.rs`) call through.
- `crates/llama-agent/src/echo.rs`, `src/test_models.rs`, `src/tests/test_utils.rs` — there is already SOME test-double machinery (an echo agent). Determine why it does NOT let tests drive the token-level generation loop deterministically. Reuse/extend rather than inventing a parallel double if possible.
- Confirm whether a trait already exists that the real model implements; if so, the fake just needs to implement the same trait. If the generation code is hard-wired to the concrete model type, the structural work is introducing the trait and routing the concrete model through it (no behavior change on the real path).

## Build

- A trait (e.g. `InferenceModel` / `TokenGenerator` — match existing naming) capturing the decode/sample step and the metadata the generation loop needs (context size, EOS token id, token→string).
- A `ScriptedModel` test double that:
  - Returns a caller-supplied sequence of tokens, then EOS.
  - Can emit an immediate EOS (0 tokens) — to reproduce/guard the 0-token bug class.
  - Can emit a tool-call token sequence mid-stream — to drive the agentic loop's tool path.
  - Lets tests assert what prompt tokens it was fed (so budget/template behavior is verifiable).
  - Has a configurable context size so the context-window guard can be exercised.
- Route the real model through the trait with ZERO behavior change on the production path (verify the existing real-model streaming tests still pass).

## Acceptance Criteria

- [ ] An inference trait exists; the real model implements it; the generation paths call through it.
- [ ] A `ScriptedModel` test double implements the trait and can: replay a token list, emit immediate EOS, emit a tool-call sequence, report fed prompt tokens, and use a configurable context size.
- [ ] The existing real-model streaming/batch tests still pass (no production behavior change) — verify `cargo test -p llama-agent` is green.
- [ ] At least one unit test drives `generate_stream` through `ScriptedModel` end-to-end with NO real model, asserting the streamed text equals the scripted tokens — proving the seam works and is fast (sub-second).

## Tests

- [ ] `scripted_model_streams_exact_tokens` — drive the streaming path with a 5-token script, assert output text + token_count.
- [ ] `scripted_model_immediate_eos_yields_empty` — script EOS first; assert 0 tokens AND that this is reported as a normal completion (this is the 0-token bug's shape; the regression cards build on it).
- [ ] Run: `cargo test -p llama-agent` — all green, new scripted tests included, no real-model download required for them.

## Workflow

- Use `/tdd`.
- Keep the real-model path byte-for-byte unchanged — this card is a refactor + test double, not a behavior change. If you can't introduce the trait without touching real behavior, stop and flag it.

## Why this is the keystone

Depends-on for the generation, queue-lifecycle, and ACP-loop coverage cards. Pure-logic cards (stopper, chat_template, ACP translation) do NOT depend on this and can proceed off the baseline measurement independently.