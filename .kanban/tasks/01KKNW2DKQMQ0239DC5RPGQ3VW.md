---
position_column: done
position_ordinal: '8480'
title: Move cosine_similarity into model-embedding crate
---
## What
Add `simsimd` dependency to `model-embedding` and provide a canonical `pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32` function. Then update all 3 production copies to re-export or import from `model-embedding`.

Currently duplicated in:
- `swissarmyhammer-treesitter/src/chunk.rs:283-294` (production)
- `swissarmyhammer-code-context/src/ops/search_code.rs:73` (production)
- `swissarmyhammer-tools/src/mcp/tools/shell/state.rs:645` (production)
- `ane-embedding/tests/integration_test.rs:150` (test helper)
- `llama-embedding/tests/integration/real_model_integration.rs:490` (test helper)
- `swissarmyhammer-embedding/tests/integration_test.rs:14` (test helper)

Steps:
1. Add `simsimd = { workspace = true }` to `model-embedding/Cargo.toml` dependencies
2. Add `pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32` in `model-embedding/src/similarity.rs` (or `lib.rs`)
3. Replace the 3 production copies with `use model_embedding::cosine_similarity`
4. Replace the 3 test copies with `use model_embedding::cosine_similarity`
5. Remove the now-dead local functions

## Acceptance Criteria
- [ ] `model-embedding` exports `cosine_similarity`
- [ ] All 3 production sites import from `model-embedding`
- [ ] All 3 test sites import from `model-embedding`
- [ ] No duplicate `cosine_similarity` implementations remain
- [ ] All tests pass across affected crates

## Tests
- [ ] `cargo nextest run -p model-embedding` — includes cosine_similarity tests (moved from duplicates)
- [ ] `cargo nextest run -p swissarmyhammer-treesitter` — still passes
- [ ] `cargo nextest run -p swissarmyhammer-code-context` — still passes
- [ ] `cargo check -p swissarmyhammer-tools` — compiles