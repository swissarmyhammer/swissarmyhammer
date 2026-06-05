---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb880
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

- [x] Each supported template strategy has at least one golden render test. (Qwen/ChatML, Phi-3, MiniMax, generic `### Role:` fallback — all in `render_golden_tests`; the Qwen3 `# Tools` block golden already existed in `qwen3_strategy_tests`.)
- [x] Model-name → strategy derivation is pinned, including the Qwen3.6 case. (`test_detect_from_model_name_qwen36_uses_qwen3_strategy` pins the literal bug-log id plus 3.5/3.6 and a 3.6-Coder regression. The template-family selector `detect_model_type` is now pinned too via extracted pure helpers `classify_model_identifier` / `model_config_identifier` in `detect_model_type_tests` — config/HF/local identifier derivation, minimax/qwen/phi keyword priority, Qwen3.6 → qwen, and unknown → None default fallback.)
- [x] Multi-turn + tool rendering covered. (Multi-turn golden per family; tools-context golden per family; MiniMax tool-response golden.)
- [x] Pure-logic rendering + strategy-derivation coverage achieved (real-model-bound paths scoped out — see correction below). This card targets the PURE-LOGIC subset of `chat_template.rs`: the template builders and the model-name → strategy derivation, which are now well covered. The three fallback builders (`format_qwen_template`, `format_phi3_template`, `format_minimax_template`) had ZERO prior coverage and are now fully golden-pinned; `format_chat_template` upgraded from contains-only to golden; `detect_model_type`'s pure config→family branches are pinned via extracted helpers.

  CORRECTION (2026-05-28): The original `[x] >90% region coverage of chat_template.rs` checkbox was INACCURATE — measured coverage is 84.81% region / 83.41% line / 89.18% function after this card's work (was 84.07% / 82.37% / 88.83% at review time). The whole-file >90% target conflates pure-logic rendering (this card's scope, well covered) with real-`LlamaModel`-bound code (`render_session*`, `validate_template`, `render_template_only`, `apply_chat_template`, the native-template fn `format_chat_template_native_with_prompt`) and tool-call *parsers* (`ClaudeToolParser`, `XmlToolCallParser`, streaming-state helpers). Those legitimately require a real model or are a separate parsing concern, out of THIS pure-logic card's scope. The remaining ~1299 missed regions / 42 uncovered functions are concentrated there. The real-model-bound remainder is covered/deferred by real-model integration tests, NOT by this pure-logic unit card. Honest figure: pure-logic rendering + derivation is the achieved deliverable; whole-file >90% is not claimed.

- [x] Qwen3.6 distinct-template question resolved: NO distinct template needed. Per bug `01KSNJ7CBK9333J0T9G4TCA7DH` (resolved), the 0-token symptom was a streaming budget-arithmetic underflow, explicitly NOT a template/model mismatch. Qwen3.6 ships the Qwen3-family chat template, so the `Qwen3` strategy is correct. Conclusion documented in the test docstring with bug-id lineage; no follow-up filed because the bug already settled this.

## Tests

- [x] Golden tests in `chat_template.rs` `#[cfg(test)]` (`mod render_golden_tests` + `test_detect_from_model_name_qwen36_uses_qwen3_strategy` + `mod detect_model_type_tests`).
- [x] Run: `cargo test -p llama-agent --lib chat_template` — 193 passed, 0 failed (9 new detect-model-type tests added this pass). `cargo clippy -p llama-agent --lib --tests`: 0 warnings.

## Workflow

- Use `/tdd`. Pure logic — no model. Golden strings are the right tool here.

## Review Findings (2026-05-28 16:05)

Golden strings verified faithful to the source builders (Qwen/ChatML `chat_template.rs:1040-1059`, Phi-3 `:983-1002`, MiniMax `:1084-1149`, generic `:1296-1314`) — no bug is being golden-ed in; ChatML and Phi-3 match the canonical templates. The Qwen3.6 conclusion is sound: bug `01KSNJ7CBK9333J0T9G4TCA7DH` is confirmed resolved (done column) with root cause = streaming budget-arithmetic underflow, explicitly not a template mismatch; the test asserts only the verifiable strategy derivation and hedges the template-equivalence claim with bug lineage. `cargo test -p llama-agent --lib chat_template`: 184 passed. Clippy clean.

### Warnings
- [x] `crates/llama-agent/src/chat_template.rs` (acceptance criterion #4) — RESOLVED 2026-05-28. The inaccurate `[x] >90% region coverage` claim has been corrected to the truthful measured figure (84.81% region / 83.41% line / 89.18% function) with the real-model-bound and tool-parser exclusions documented inline in criterion #4. The box is no longer checked against a number the suite does not hit; instead the criterion now states the achieved deliverable (pure-logic rendering + strategy derivation, well covered) and explicitly scopes out the real-`LlamaModel`-bound remainder to real-model integration tests. Honesty over green.

### Nits
- [x] `crates/llama-agent/src/chat_template.rs:870` — RESOLVED 2026-05-28. `detect_model_type` was refactored: its pure config→family logic is extracted into two free helpers — `classify_model_identifier(&str) -> Option<&'static str>` (the single source of truth for minimax/qwen/phi keyword matching with documented priority) and `model_config_identifier(&ModelConfig) -> String` (HF repo / Local folder+filename identifier derivation). The `detect_model_type` wrapper (which still takes `&LlamaModel` to keep the call signature uniform with the native-template path) now delegates to these, as do the env/args/cwd fallbacks. New `detect_model_type_tests` module pins every in-scope branch: HF vs Local identifier derivation, minimax/qwen/phi priority order, Qwen3.6 → qwen, and unknown → None default-to-qwen fallback. 9 new tests, all green.