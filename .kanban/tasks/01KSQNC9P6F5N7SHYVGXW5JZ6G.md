---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
position_column: todo
position_ordinal: '9280'
project: llama-coverage
title: 'Bug+dedup: stale duplicate streaming loop in generator.rs carries the fixed 0-token bugs (0% covered)'
---
## What

The streaming token-budget / chunk-accounting bugs fixed in production (commit `16f7aad5a`, bug `01KSNJ7CBK9333J0T9G4TCA7DH`) exist in a **second, stale copy** of the streaming loop that the production queue path does NOT use — `crates/llama-agent/src/generation/generator.rs::generate_stream_with_context` (~lines 704–885), the `TextGenerator` impl on `LlamaCppGenerator`. Baseline coverage measured this file at **0.00%**, so nothing exercises it.

### Confirmed: NOT a live production regression

Traced the production path: ACP server → `Agent::generate_stream` (agent.rs:1150) → `request_queue.submit_streaming_request` → queue worker → `queue.rs:1151` → `GenerationHelper::generate_stream_with_borrowed_model_and_template_offset` in `generation/mod.rs` (the FIXED path). `generator.rs::generate_stream` is a separate impl the queue never calls. This card is a latent-landmine cleanup + dedup, not a production hotfix.

### The bugs in generator.rs::generate_stream_with_context

1. **Double push** — `generated_text.push_str(&token_text)` at line ~790 (lossy decode) AND again at ~795 (`token_to_str`, shadowed var). Accumulated text is doubled. (mod.rs fix removed exactly this duplicate.)
2. **Cumulative per-chunk count** — `token_count: tokens_generated` at line ~801 (triangular-number bug). Should be a per-chunk delta of 1. (mod.rs fix changed this.)
3. **Gated send** — the chunk is only sent inside `if let Ok(token_text) = self.model.token_to_str(...)` at ~794, so a token that needs lossy decoding is counted (line 791) but never streamed → text/count mismatch + dropped output.
4. No max-tokens budget guard equivalent to the mod.rs context-window guard (verify).

### Bigger issue: multiple drifting copies

Comments in generator.rs say "matches queue.rs:881-979" and "matches queue.rs:924-932" — evidence of at least three hand-copied streaming loops (mod.rs, generator.rs, and a historical queue.rs copy) that drift independently. The original bug took two rounds partly because of this duplication. **Prefer deduplication over fixing the copy in place.**

## Approach (investigate, then choose)

1. Determine whether `generator.rs::generate_stream` / `LlamaCppGenerator` is reachable in ANY production or test path, or is fully dead.
   - If **dead**: delete `generate_stream_with_context` (and the dead `generate_stream` impl) rather than fixing it — no point maintaining a buggy unused copy. Confirm no caller breaks.
   - If **reachable** by some path: fix all three bugs to match the mod.rs implementation, and add coverage (see the generation-core card `01KSQBEAVG5FCXF3TT411A88Z7`).
2. Either way, collapse the duplication: there should be ONE streaming loop implementation. If mod.rs's `generate_stream_with_borrowed_model` is canonical, route/delete the others toward it.

## Acceptance Criteria

- [ ] `generator.rs` no longer contains an independent streaming loop with the double-push / cumulative-count / gated-send bugs — either deleted (if dead) or fixed-and-deduplicated against the canonical mod.rs loop.
- [ ] If deleted: confirm nothing references it; the crate builds and all tests pass.
- [ ] If kept: a test (real small model, since this binds llama.cpp) proves non-empty output + per-chunk count of 1 + budget honored, mirroring the mod.rs streaming regression tests.
- [ ] A short note in the code (or this task) documents which streaming loop is canonical, so future copies aren't made.

## Tests

- [ ] If kept: real-model streaming test through `LlamaCppGenerator::generate_stream` asserting content (tokens > 0, non-empty, per-chunk count correct).
- [ ] If deleted: `cargo test -p llama-agent` green with the dead code removed.
- [ ] Run: `cargo test -p llama-agent` and `cargo clippy -p llama-agent --all-targets -- -D warnings`.

## Workflow

- Use `/explore` first to settle the dead-vs-reachable question via the call graph before touching code.
- Lineage: found during the llama-coverage keystone review (`01KSQBDM9M4RJJYGQDTZYJA107`); same bug family as `01KSNJ7CBK9333J0T9G4TCA7DH`.