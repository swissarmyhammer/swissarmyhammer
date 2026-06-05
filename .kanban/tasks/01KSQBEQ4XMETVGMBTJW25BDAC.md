---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb580
project: llama-coverage
title: Cover stop conditions (stopper/eos.rs, max_tokens.rs, mod.rs) — pure logic, no model
---
## What

The `crates/llama-agent/src/stopper/` module decides when generation halts. It is pure logic — no model needed — and a stop bug silently truncates or runs away. Cover it exhaustively.

## Cover

- `stopper/eos.rs` — EOS token detection: the EOS id, a non-EOS id, and any model-specific alternate end tokens.
- `stopper/max_tokens.rs` — boundary: stops at exactly N, not N-1, not N+1.
- `stopper/mod.rs` — the composite/dispatch: when multiple stoppers are active, the first to fire wins; ordering and precedence.
- **Stop sequence straddling a chunk boundary** — if stop-string matching exists, a stop sequence split across two decode steps must still be detected. (If string-stop lives elsewhere, note where and cover it there.)

## Acceptance Criteria

- [x] Each stopper type has explicit boundary tests (fires / does-not-fire at the edges).
- [x] The composite stopper's precedence is pinned.
- [x] `stopper/` region coverage reaches the epic threshold (target >95%).
- [x] No real model used — these are pure predicate tests.

## Tests

- [x] Unit tests colocated in each `stopper/*.rs` `#[cfg(test)]` module or a `stopper/tests.rs`.
- [x] Run: `cargo test -p llama-agent stopper` and confirm the coverage delta for `stopper/`.

## Workflow

- Use `/tdd`. This card is independent of the scripted-model harness — pure predicates.

## Implementation Notes

- **String-stop lives elsewhere**: stop-sequence (string) matching is NOT in `stopper/`. It is `GenerationProcess::should_stop(&self, generated_text, stop_tokens)` at `crates/llama-agent/src/generation/generator.rs:184`, which does `generated_text.contains(stop_token)` against the *accumulated* generated text. Because it matches the full running string (not a per-chunk slice), a stop sequence split across two decode steps is inherently detected once both chunks are appended. That file is owned by a concurrent agent (`generation/+model.rs`), so per the disjoint-files constraint it was documented here rather than edited.
- **`LlamaContext` blocker**: the `Stopper::should_stop` trait method takes `&LlamaContext`, which cannot be constructed without loading a model. To honor "no real model — pure predicates", the model-independent decision logic was factored into context-free cores: `MaxTokensStopper::record_tokens(usize)` and `EosStopper::evaluate()`. `should_stop` is now a thin wrapper that reads `batch.n_tokens()` / borrows `context.model` and delegates. All boundary, overflow, and precedence behavior is tested against these cores.
- **`MaxTokensStopper` overflow fix**: changed `self.tokens_generated += tokens_in_batch` to `wrapping_add` so the defensive overflow branch is actually reachable (in debug builds `+=` panics on overflow before the guard runs, making the branch dead). The overflow guard is now covered by `record_tokens_detects_counter_overflow`.
- **Composite precedence** (`mod.rs`): there is no composite type in `stopper/`; the live dispatch is the ordered `Vec<Box<dyn Stopper>>` loop in `generator.rs` (first `Some` wins, then `break`). `mod.rs` pins that "first-to-fire-wins" rule via `first_to_fire` over ordered `Option<FinishReason>` values, including a test driving the real stopper cores in production order (MaxTokens before Eos).
- **`LlamaBatch` is model-free**: confirmed `LlamaBatch::new`/`add`/`n_tokens` need no model; `should_stop_delegates_to_record_tokens_via_batch` builds and populates a real batch to pin the wiring.

## Results

- `cargo test -p llama-agent --lib stopper`: 33 passed, 0 failed (was 9). `cargo test -p llama-agent --doc stopper`: 13 passed.
- Coverage (cargo llvm-cov, lib): `mod.rs` 100% regions/lines; `max_tokens.rs` 96.43% regions / 95.77% lines; `eos.rs` 93.53% regions / 92.39% lines. Aggregate over the `stopper/` region ≈96.1% regions / ≈95.3% lines, above the >95% target. The only uncovered regions are the `should_stop`/`as_any_mut` trait wrappers that require a real `LlamaContext` (impossible model-free) and one `debug!` format-args region.
- `cargo clippy -p llama-agent --lib --tests`: clean, zero warnings.
- Changes confined to `crates/llama-agent/src/stopper/{eos,max_tokens,mod}.rs`; disjoint files left untouched.