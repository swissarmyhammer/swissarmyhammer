---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: todo
position_ordinal: 8c80
project: llama-coverage
title: Cover chat-template rendering (chat_template.rs) across model families — pure logic
---
## What

`crates/llama-agent/src/chat_template.rs` is the largest file in the crate (8.3k lines) and is pure string transformation — prompt messages → the model-specific rendered prompt. A template bug produces a malformed prompt, which is a prime suspect for garbage/empty generation on real models. Cover the rendering for each supported strategy.

## Cover

- **Per-strategy rendering** — every template strategy the code supports (Qwen3, and whatever else: ChatML, Llama, Mistral, GLM, etc.). For each: a known message list renders to the exact expected string (golden tests).
- **The Qwen3 vs Qwen3.6 question** — the 0-token bug investigation noted `strategy: Some(Qwen3)` was derived for a `Qwen3.6-27B` model. Confirm whether the Qwen3 template is actually correct for Qwen3.6 weights, or whether 3.6 needs its own. Pin the derivation logic (model name → strategy) with tests, and add a 3.6 case.
- **Multi-turn** — system + user + assistant + tool messages render with correct role markers and special tokens.
- **Tool/function-call formatting** — if the template injects tool schemas, cover that rendering.
- **Edge cases** — empty message list, system-only, unknown role, very long content.
- **Strategy derivation fallback** — an unknown model name falls back to a sane default (and that default is documented).

## Acceptance Criteria

- [ ] Each supported template strategy has at least one golden render test.
- [ ] Model-name → strategy derivation is pinned, including the Qwen3.6 case.
- [ ] Multi-turn + tool rendering covered.
- [ ] `chat_template.rs` region coverage reaches the epic threshold (target >90% given its size; justify exclusions).
- [ ] If Qwen3.6 needs a distinct template, EITHER fix it here OR file a precise follow-up and note it in the qwen 0-token bug (`01KSNJ7CBK9333J0T9G4TCA7DH`) lineage.

## Tests

- [ ] Golden tests in `chat_template.rs` `#[cfg(test)]` or a `chat_template/tests.rs`.
- [ ] Run: `cargo test -p llama-agent chat_template` and confirm the coverage delta.

## Workflow

- Use `/tdd`. Pure logic — no model. Golden strings are the right tool here.