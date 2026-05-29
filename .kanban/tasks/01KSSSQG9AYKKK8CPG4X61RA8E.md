---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: Harden streaming KV-reuse test + dedupe offset helpers (review nits)
---
Follow-up from review of d4a69cbe8 (card 01KSSS5H82YC0TX0CM6SQV8CRP, test concern + nits). Low priority.

1. `tests/integration/streaming_generation.rs::test_streaming_reuses_kv_cache_across_turns` installs a GLOBAL tracing subscriber and `return`s early (skips all asserts) if one is already installed. Under nextest (process-per-test) this is fine; under shared-process `cargo test` it can pass VACUOUSLY. Make it impossible to pass without asserting — e.g. fail if the install failed, or scope the capture so it can't no-op.

2. `streaming_offset_decision` duplicates the `kv_pos < 0` / `+1` logic of `compute_template_token_count` with an added upper-bound guard. Fold into one shared function parameterized on whether the upper-bound check applies, so batch and streaming offset rules can't drift.

3. `prepare_streaming_kv_cache` tokenizes the prompt to get `total`, then the generation fn tokenizes the same prompt again — redundant full tokenization per turn on large prompts. Tokenize once and thread the token slice through.