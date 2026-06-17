---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvbq3kmf1d44s76cg0kvswh1
  text: Picked up by /finish. Keystone zckz9va (max_rollback field + rollback-aware selector) is done and committed (0abcb1aaa), so the field this card wires into now exists. Starting /implement.
  timestamp: 2026-06-17T21:17:00.431494+00:00
- actor: wballard
  id: 01kvbq71vcfpbvch020tf151va
  text: |-
    Research done. Findings:

    DETECTION SIGNAL CHOSEN: model identifier string (repo/filename via model_identifier_for_strategy), keyed on Qwen version.
    - The fork DOES expose LlamaModel::is_recurrent() (llama-cpp-2/src/model.rs:496, wraps llama_model_is_recurrent), BUT it requires a live loaded LlamaModel. At RequestQueue::new the model is NOT loaded (only ModelConfig is available), so the capability bit is not cleanly available at store-construction time. Falling back to the model-config/alias signal per task instructions.
    - Production targets confirmed: builtin/models/qwen.yaml -> unsloth/Qwen3.6-35B-A3B-MTP-GGUF (hybrid attn+recurrent gated delta net = RECURRENT). Test hybrid: unsloth/Qwen3.5-0.8B-MTP-GGUF (RECURRENT). Pure attention: unsloth/Qwen3-0.6B-GGUF (Qwen3.0 = ATTENTION ONLY).
    - Distinguishing signal: Qwen3.5/3.6+ use gated delta net (recurrent); plain Qwen3 (3.0) is attention-only. model.rs N_RS_SEQ comment already documents "Qwen3.5/3.6 with gated delta net".

    PLAN:
    1. Promote N_RS_SEQ (model.rs, function-local const 64, u32) to a shared module const used by both context-params builder and queue.
    2. Add pure fn recurrent_rollback_window(identifier: &str) -> usize returning N_RS_SEQ (64) for recurrent/hybrid (qwen3.5/3.6+) else usize::MAX.
    3. Wire into RequestQueue::new only; leave #[cfg(test)] with_executor at usize::MAX.

    TDD: write failing helper test first.
  timestamp: 2026-06-17T21:18:53.292884+00:00
- actor: wballard
  id: 01kvbqy71dw5xj3qaxpabnz4th
  text: |-
    IMPLEMENTED + GREEN. Moving to review.

    Files changed:
    - crates/llama-agent/src/model.rs: promoted function-local `const N_RS_SEQ: u32 = 64` to a single module-level `pub(crate) const N_RS_SEQ: u32 = 64`, moving the rationale comment to the const. Context-params builder still uses `.with_n_rs_seq(N_RS_SEQ)` — no duplicated literal.
    - crates/llama-agent/src/agent.rs: added pure `recurrent_rollback_window(identifier: &str) -> usize` (+ private `is_recurrent_model_identifier`). Returns `crate::model::N_RS_SEQ as usize` (64) for hybrid/recurrent (Qwen3.5/3.6+ gated delta net), `usize::MAX` for attention-only. Plus 2 unit tests (+1 multi-digit-minor case).
    - crates/llama-agent/src/queue.rs: wired `recurrent_rollback_window(&model_identifier)` into the PRODUCTION store at `RequestQueue::new`. The `#[cfg(test)] with_executor` site stays `usize::MAX`.

    DETECTION SIGNAL: model identifier string (repo/filename), keyed on Qwen version. The fork exposes `LlamaModel::is_recurrent()` but it needs a loaded model, which isn't available at store-construction in `RequestQueue::new` — so per task instructions, fell back to the alias/arch signal. Qwen3.5/3.6+ (gated delta net) are recurrent; plain Qwen3 (3.0) is attention-only. Full numeric minor parsed (qwen3.10 reads as 10, not 1).

    RED->GREEN evidence:
    - RED: `cargo test -p llama-agent --lib recurrent_rollback_window` failed to compile — `N_RS_SEQ not found in crate::model` + `cannot find function recurrent_rollback_window` (feature missing, not typo).
    - GREEN: both helper tests pass; multi-digit-minor case added and passing.

    Verification gate (all green):
    - cargo fmt -p llama-agent: applied.
    - cargo clippy -p llama-agent --all-targets -- -D warnings: clean (fixed a doc_lazy_continuation lint on the promoted doc comment by separating the list from the following paragraph).
    - cargo test -p llama-agent --lib: 1112 passed; 0 failed (1110 baseline + 2 new).
    - cargo build -p llama-agent: clean.

    Adversarial double-check: PASS (verified single-source 64, all four classification cases, production wiring site, non-tautological tests). Its forward-looking caveat about a hypothetical Qwen3.10 single-digit misclassification was hardened away (full numeric-run parse + dedicated test).
  timestamp: 2026-06-17T21:31:32.269460+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbe80
project: kv-prefix-reuse
title: Source the recurrent rollback window from the model into the session-state store
---
## What
Wire the real recurrent-state rollback window into the `SessionStateStore.max_rollback` field added by the donor-selection keystone, so production hybrid/recurrent models (Qwen3.6-35B-A3B MoE) get rollback-aware selection, while pure-attention models keep `usize::MAX`.

In `crates/llama-agent/src/queue.rs`, `RequestQueue::new` (`queue.rs:1207`) builds the store at `queue.rs:1225` and already holds `model_manager` (with `get_config()`), mirroring how it derives `model_identifier_for_strategy`. Determine whether the loaded model is recurrent/hybrid and, if so, pass the `n_rs_seq` window (`N_RS_SEQ = 64`); otherwise pass `usize::MAX`.

- `N_RS_SEQ` is currently a **function-local `const`** inside the context-params builder in `crates/llama-agent/src/model.rs:436` — promote it to a module/associated const (or accessor) so the queue and the context-params builder share ONE definition; do not duplicate the literal `64`.
- Recurrent detection: prefer a model-capability check (the llama-cpp-rs fork may expose `llama_model_is_recurrent`/arch flags). If no clean capability bit is available at store-construction time, fall back to a model-config flag derived from the model alias/arch string.
- Only the PRODUCTION store at `queue.rs:1225` (`RequestQueue::new`) gets the model-derived window. The `#[cfg(test)] with_executor` site stays `usize::MAX`.

## Acceptance Criteria
- [x] For the qwen hybrid model, the production `SessionStateStore` is constructed with `max_rollback == N_RS_SEQ` (64).
- [x] For a pure-attention model, the store is constructed with `max_rollback == usize::MAX`.
- [x] `N_RS_SEQ` has a single shared definition (model.rs:60 `pub(crate) const`, doc-commented) used by both the context-params builder and the queue.
- [x] No ACP-layer or protocol changes.

## Tests
- [x] `recurrent_rollback_window(&str)` unit tests (agent.rs): 64 for Qwen3.5/3.6/3.10 recurrent descriptors, usize::MAX for Qwen3.0 / non-Qwen. RED (didn't compile — symbol absent) → GREEN.
- [x] `cargo test -p llama-agent --lib` → 1112 passed; clippy -D warnings clean; build clean.

## Decision: detection signal
Keyed on the model identifier string (alias/arch), not the fork's `LlamaModel::is_recurrent()` — that needs a loaded model, unavailable at store-construction time in `RequestQueue::new` (only `ModelConfig` is). Documented fallback per the card.

## Workflow
- Use `/tdd` — write the detection-helper test first (RED), then implement and wire it into `RequestQueue::new`.

## Review Findings (2026-06-17 16:32)

> ⚠️ Engine self-reported 1/15 failed — results INCOMPLETE. Line numbers in this run are systematically wrong (verified below).

**Orchestrator triage — no in-scope actionable defects; advanced to done:**
- [ ] ~~Blocker `agent.rs:2822` — no-op match test~~ PRE-EXISTING `test_agent_server_creation` (next to `test_agent_server_debug`), not added by this card → out of scope.
- [ ] ~~Warning `model.rs:201` — deep nesting in metadata parsing~~ PRE-EXISTING code, untouched by this card → out of scope.
- [ ] ~~Nit `agent.rs:587` — `recurrent_rollback_window` missing doc comment~~ ALREADY DOCUMENTED (agent.rs:102-116); engine cited wrong line.
- [ ] ~~Nit `model.rs:621` — `N_RS_SEQ` missing doc comment~~ ALREADY DOCUMENTED (model.rs:57-59); engine cited wrong line (621 is session cleanup).
- [ ] ~~Nits `agent.rs:2761`, `model.rs:376`, `model.rs:413` — magic numbers (512/8192/4)~~ PRE-EXISTING fixtures/config, out of scope.
- [ ] Nit `agent.rs:130` — inline single-call-site `is_recurrent_model_identifier`. Debatable style nit on documented code; left as a deliberately-split, separately-meaningful predicate. Not worth a churn round.