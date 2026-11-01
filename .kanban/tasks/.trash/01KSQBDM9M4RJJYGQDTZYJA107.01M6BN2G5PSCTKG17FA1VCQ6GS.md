---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb780
project: llama-coverage
title: Scripted fake model behind an inference trait (the deterministic test keystone)
---
## What

The only part of llama-agent that genuinely needs a real model is the FFI decode (feed tokens → llama.cpp → logits). Everything above it — budget arithmetic, the streaming loop, stop conditions, chunk accounting, the queue worker, the ACP agentic turn loop — is deterministic logic that should be driven without a GPU or weights.

Build the seam that makes that possible: an **inference trait** that abstracts the per-step "given the current token context, produce the next token (or EOS)" operation, plus a **scripted fake model** implementing it that returns predetermined tokens. This is the keystone the model-dependent coverage cards depend on.

## Implementation notes (resolved)

Investigation found that the **inference trait already exists**: `crate::generation::TextGenerator` (generation/mod.rs). The real model implements it via `LlamaCppGenerator` (generation/generator.rs), and the generation paths call through it. It abstracts the whole decode/sample/stream operation (`generate_text`, `generate_stream`, `*_with_context`, `*_with_template_offset`).

A lower per-step trait that the SHARED production loop calls through was considered and rejected: the production loop binds `LlamaContext`/`LlamaBatch`/`LlamaSampler` and the `Stopper` trait (`should_stop(&LlamaContext, &LlamaBatch)`) together; extracting a per-step seam without changing the real path's byte-for-byte behavior would require making `Stopper` generic — colliding with the concurrent stopper card (01KSQBEQ4XMETVGMBTJW25BDAC) and risking the "real path unchanged" guarantee. The interactive design question was declined, so the lower-risk, criteria-satisfying option was taken.

Outcome: `ScriptedModel` implements the existing `TextGenerator` trait. **Zero production-path change** — the only edits are a new gated module (`generation/scripted.rs`) plus two additive re-exports. `ScriptedModel`'s streaming loop faithfully mirrors the production `GenerationHelper` chunk contract (per-token `{token_count: 1}` chunks, completion `{text:"", is_complete:true, token_count:0, finish_reason: Stopped(reason)}`, and the same reason strings: EndOfSequence / MaxTokens / StopToken / ContextWindowFull).

Files: `crates/llama-agent/src/generation/scripted.rs` (new), `crates/llama-agent/src/generation/mod.rs` (module decl + re-export, gated on `any(test, feature="test-utils")`), `crates/llama-agent/src/lib.rs` (re-export, same gate).

## Build

- A trait capturing the decode/sample step and metadata — satisfied by the existing `TextGenerator` trait.
- A `ScriptedModel` test double that:
  - Returns a caller-supplied sequence of tokens, then EOS.
  - Can emit an immediate EOS (0 tokens) — to reproduce/guard the 0-token bug class.
  - Can emit a tool-call token sequence mid-stream — to drive the agentic loop's tool path.
  - Lets tests assert what prompt tokens it was fed (`fed_prompts()` / `last_prompt()`).
  - Has a configurable context size (`with_context_size`) so the context-window guard can be exercised.

## Acceptance Criteria

- [x] An inference trait exists; the real model implements it; the generation paths call through it. (`TextGenerator` — pre-existing, implemented by `LlamaCppGenerator`.)
- [x] A `ScriptedModel` test double implements the trait and can: replay a token list, emit immediate EOS, emit a tool-call sequence, report fed prompt tokens, and use a configurable context size.
- [x] The existing real-model streaming/batch tests still pass (no production behavior change) — `cargo test -p llama-agent` green (lib 897 passed / 0 failed, doctests 38 passed, integration tests 0 failed).
- [x] At least one unit test drives `generate_stream` through `ScriptedModel` end-to-end with NO real model, asserting the streamed text equals the scripted tokens — `scripted_model_streams_exact_tokens`, runs in 0.00s.

## Tests

- [x] `scripted_model_streams_exact_tokens` — drives the streaming path with a 5-token script, asserts output text + token_count.
- [x] `scripted_model_immediate_eos_yields_empty` — scripts EOS first; asserts 0 tokens AND that this is a normal completion (Ok, reason EndOfSequence) — the 0-token bug's shape.
- [x] Run: `cargo test -p llama-agent` — all green, new scripted tests included (8 total in the scripted module), no real-model download required for them.
- [x] Bonus coverage: tool-call mid-stream, MaxTokens budget cap, stop-token honoring, context-window guard, batch `generate_text`, fed-prompt recording.
- [x] `cargo clippy -p llama-agent --all-targets --features test-utils` — zero warnings.

## Workflow

- Use `/tdd`.
- Keep the real-model path byte-for-byte unchanged — this card is a refactor + test double, not a behavior change. If you can't introduce the trait without touching real behavior, stop and flag it.

## Why this is the keystone

Depends-on for the generation, queue-lifecycle, and ACP-loop coverage cards. Pure-logic cards (stopper, chat_template, ACP translation) do NOT depend on this and can proceed off the baseline measurement independently.

## Review Findings (2026-05-28 16:05)

Verified independently: 8/8 scripted tests pass (incl. both named acceptance tests, asserting on content); `cargo clippy -p llama-agent --features test-utils --lib` clean; the uncommitted production edits are only mod.rs +7 / lib.rs +5 (both gated re-exports) — the real path is byte-for-byte unchanged (the mod.rs budget fix is prior commit 16f7aad5a, not this card's work). The `TextGenerator`-level design decision is sound and correctly avoids the Stopper-generic collision. This card's own deliverable is clean. The one finding below is NOT a defect in this card — it is a scope correction the downstream generation-core card requires, surfaced here because this card is its keystone.

### Warnings
- [ ] Rescope downstream generation-core card `01KSQBEAVG5FCXF3TT411A88Z7` — `ScriptedModel` cannot meet its goal. That card targets ">95% region coverage of `generation/mod.rs` + `generation/generator.rs` via ScriptedModel" and lists `generate_stream_with_borrowed_model`, the offset variant, `generate_common`, `token_to_str_lossy`, and the context-window-guard boundary. None of those are reachable through the `TextGenerator` trait — they are `GenerationHelper` / free-fn signatures taking `&LlamaModel` / `&mut LlamaContext` directly, and `LlamaCppGenerator::generate_stream_with_context` (generator.rs:704-885) is its own decode loop holding the real model/context. A `ScriptedModel` test exercises ScriptedModel's *reimplementation* of the contract, not the production arithmetic, so it produces zero region coverage of those functions. Concretely, `generator.rs::generate_stream_with_context` still has two live bugs ScriptedModel structurally cannot catch: (a) double-push of every token into `generated_text` (lines 790 and 794-795), and (b) `token_count: tokens_generated` (line 801) emitting the cumulative running total per chunk instead of `1` — the exact triangular-number regression that card says it wants to guard against. Resolution for that card: either (1) extract the budget/chunk-accounting/context-guard arithmetic into pure functions that both the real decode loops and the tests call (then Scripted- or unit-test those pure functions for real coverage), or (2) cover `mod.rs`/`generator.rs` with the small real model per the epic's "FFI is the legitimately-real part" stance — and reconcile the two divergent streaming loops (mod.rs `GenerationHelper` is fixed; generator.rs is not). Update card `01KSQBEAVG5FCXF3TT411A88Z7`'s acceptance criteria accordingly before it is started.