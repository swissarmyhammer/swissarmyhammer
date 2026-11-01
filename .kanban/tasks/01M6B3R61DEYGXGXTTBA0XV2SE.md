---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bm495cm6ztrz25e3h9cq0k
  text: |-
    ### implement — changed
    - evidence: deleted builtin/models/nomic-embed-code.yaml; builtin/models/qwen-embedding.yaml (ane-embedding only); edited crates/swissarmyhammer-config tests (builtin_agents_generation.rs, model_configs.rs, model/tests/manager.rs, model/chat_model_config_tests.rs); crates/swissarmyhammer-embedding/src/embedder.rs (removed nomic unit tests that built a llama embedder); crates/swissarmyhammer-embedding/tests/integration_test.rs (new real-path test qwen_embedding_embeds_one_text_on_ane). Ran cargo test -p swissarmyhammer-config (pass), -p swissarmyhammer-embedding (pass), and integration_test with --features embedding-models (4 pass on Mac ANE).
    - note: DEFAULT_MODEL_NAME already is qwen-embedding. Benches and convert scripts still mention nomic; they are outside the card list.
    - next: /review
  timestamp: 2026-11-01T13:17:36.812884+00:00
- actor: claude-code
  id: 01m6bmdd60bbcnszba5fyxjpe6
  text: |-
    ### test — green
    - evidence: cargo nextest run --override-version-check -p swissarmyhammer-config -p swissarmyhammer-embedding -p swissarmyhammer-kanban -p swissarmyhammer-tools -p swissarmyhammer-treesitter: 3386 run, 3386 passed, 0 skipped. cargo clippy --all-targets (same 5 crates) -- -D warnings: first run 1 error (clippy::single_element_loop in crates/swissarmyhammer-config/src/model/chat_model_config_tests.rs); fixed (loop removed, doc comment changed from "both embedding YAMLs" to "the embedding YAML"); second run clean. Config crate re-run after fix: 399 run, 399 passed, 0 skipped. cargo fmt --check clean.
    - next: review
  timestamp: 2026-11-01T13:22:35.840950+00:00
- actor: claude-code
  id: 01m6bme7vk9c42yx11yn8b89d2
  text: |-
    ### commit — changed
    - evidence: efc5d0142 feat(embedding): make Qwen3 on ANE the only builtin embedding model (8 files: builtin/models/nomic-embed-code.yaml deleted, builtin/models/qwen-embedding.yaml, swissarmyhammer-config tests, swissarmyhammer-embedding src and tests)
    - next: review. Kanban files in .kanban/ were left unstaged on purpose, per the caller instruction.
  timestamp: 2026-11-01T13:23:03.155668+00:00
- actor: claude-code
  id: 01m6bmkr6h0seq01byqztqwcxb
  text: |-
    ### review — findings
    - evidence: 2 findings (2 confirmed, 0 refuted): crates/swissarmyhammer-embedding/tests/integration_test.rs:108 (reuse/reuse), crates/swissarmyhammer-embedding/tests/integration_test.rs:122 (code-hygiene/magic-numbers)
    - next: implementer fixes both items in the Review Findings section; task stays in review
  timestamp: 2026-11-01T13:26:03.729202+00:00
- actor: claude-code
  id: 01m6bmm5r065qhmprjz1tb2fbm
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 8 files
    - test: green — cargo nextest, 3386 passed
    - commit: efc5d0142
    - review: findings — crates/swissarmyhammer-embedding/tests/integration_test.rs:108, crates/swissarmyhammer-embedding/tests/integration_test.rs:122
  timestamp: 2026-11-01T13:26:17.600476+00:00
- actor: claude-code
  id: 01m6bmnc3410ne6fa2tqr78zj3
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-embedding/tests/integration_test.rs. Removed the duplicate ANE test. Moved the macOS-only backend_name "ane" assertion (cfg-gated) into qwen_embedding_load_and_embed. Added shared helper load_qwen_embedder and constants QWEN3_EMBEDDING_DIM and QWEN_MODEL_NAME; all 3 tests use them. No bare 1024 or repeated model-name setup remains in the file. Ran: cargo test -p swissarmyhammer-embedding --features embedding-models --test integration_test — 3 passed, 0 failed.
    - next: review
  timestamp: 2026-11-01T13:26:56.868551+00:00
- actor: claude-code
  id: 01m6bn94j386jcceqxxkkemnzs
  text: |-
    ### test — green
    - evidence: cargo nextest run -p swissarmyhammer-embedding: 28 passed, 0 failed, 0 skipped; cargo nextest run -p swissarmyhammer-embedding --features embedding-models --test integration_test: 3 passed, 0 failed, 0 skipped; cargo clippy -p swissarmyhammer-embedding --all-targets -- -D warnings: clean; same with --features embedding-models: clean; cargo fmt -p swissarmyhammer-embedding --check: clean
    - next: review
  timestamp: 2026-11-01T13:37:44.515469+00:00
- actor: claude-code
  id: 01m6bna1qfmezcaamj72ajmp0n
  text: |-
    ### commit — changed
    - evidence: 034feea83 test(embedding): share one helper across the integration tests. Files: crates/swissarmyhammer-embedding/tests/integration_test.rs, .kanban/tasks/01M6B3R61DEYGXGXTTBA0XV2SE.jsonl, .kanban/tasks/01M6B3R61DEYGXGXTTBA0XV2SE.md
    - next: review
  timestamp: 2026-11-01T13:38:14.383447+00:00
- actor: claude-code
  id: 01m6bndvzx69tgg16x4k50gkr1
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (f8a333ee0): 0 findings, 0 confirmed, 0 refuted. Prior item 1 fixed: duplicate test removed, shared helper load_qwen_embedder in crates/swissarmyhammer-embedding/tests/integration_test.rs, ANE assertion cfg-gated inside qwen_embedding_load_and_embed. Prior item 2 fixed: const QWEN3_EMBEDDING_DIM at line 25, no bare 1024 left.
    - next: none. Task moved to done.
  timestamp: 2026-11-01T13:40:19.581660+00:00
- actor: claude-code
  id: 01m6bne9cqh540dx79qf0eezp2
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 1 file
    - test: green — cargo nextest, 28 passed plus 3 integration passed
    - commit: f8a333ee0
    - review: clean — 0 findings, task moved to done
  timestamp: 2026-11-01T13:40:33.303772+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd480
title: Make Qwen3 on ANE the only builtin embedding model
---
Goal: no builtin embedding model needs llama.cpp.

Decision (user approved): delete `nomic-embed-code`. It has no CoreML build. CoreML takes about 1.85s per embed. llama.cpp takes about 66ms. Do not convert it.

Steps:
- Delete `builtin/models/nomic-embed-code.yaml`.
- In `builtin/models/qwen-embedding.yaml`: keep the `ane-embedding` executor for repo `wballard/Qwen3-Embedding-0.6B-CoreML`. Remove the `llama-embedding` fallback executor and its GGUF repo.
- Remove every reference to `nomic-embed-code`. Known places: `crates/swissarmyhammer-config/tests/integration/builtin_agents_generation.rs`, `crates/swissarmyhammer-config/tests/integration/model_configs.rs`, `crates/swissarmyhammer-config/src/model/tests/manager.rs`, `crates/swissarmyhammer-config/src/model/chat_model_config_tests.rs`, `crates/swissarmyhammer-embedding/src/embedder.rs`.
- Check that the default embedding model name resolves to qwen-embedding.
- Add a real-path test: load qwen-embedding through `Embedder` and embed one text on the Mac runner.

Done when: only `qwen-embedding.yaml` exists as a builtin embedding model, it names only `ane-embedding`, and the real-path test passes. #remove-llama-cpp

## Review Findings (2026-11-01 07:23)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 6 file(s) reviewed, 2 not reviewed.

> 2 file(s) not reviewed — no validator matched:
> - `builtin/models/nomic-embed-code.yaml` — no validator matches this file
> - `builtin/models/qwen-embedding.yaml` — no validator matches this file

- [x] `crates/swissarmyhammer-embedding/tests/integration_test.rs:108` `reuse/reuse` — The new test qwen_embedding_embeds_one_text_on_ane repeats the steps of the existing qwen_embedding_load_and_embed test in the same file: it builds the embedder with Embedder::from_model_name("qwen-embedding"), loads it, embeds one text, and asserts a 1024-dimension result. The only additions are a backend_name assertion and a macOS-only cfg gate. A parallel copy of the same sequence will drift from the original as the setup changes. Give the existing test the backend_name assertion, or move the shared load-and-embed steps into one helper that both tests call. If the macOS-only gate is needed, put the cfg on the ANE-specific assertion or on a thin wrapper around the shared helper, not on a second copy of the flow.
- [x] `crates/swissarmyhammer-embedding/tests/integration_test.rs:122` `code-hygiene/magic-numbers` — The literal 1024 (the Qwen3 embedding dimension) is written again in a new assertion. The same value already appears at line 41 in the existing Qwen test. A change to the dimension must now be made at both sites, and no shared name records what the value means. Define one constant, for example `const QWEN3_EMBEDDING_DIM: usize = 1024;` at the top of the file, and use it in both assertions. Use the constant's type to match `result.dimension()`.
