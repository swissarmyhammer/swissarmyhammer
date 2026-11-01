---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bnm66dhg587rzshdr59why
  text: |-
    ### implement — changed
    - evidence: 17 files. embedding crate: Cargo.toml (llama-embedding removed, ane-embedding now a normal dependency), src/embedder.rs (Llama variant, build_llama_model, convert_source and their tests removed; backend enum has one macOS-only Ane variant), src/lib.rs docs. config crate: src/model.rs (LlamaEmbedding executor type and config variant removed, docs updated), src/model/chat_model_config_tests.rs, src/model/tests.rs, src/model/tests/{config_structure,loading,manager,parsing,resolution,serialization,types}.rs, tests/integration/model_configs.rs. Cargo.lock lost the llama-embedding edge.
    - new tests (RED first, then GREEN): parsing.rs test_parse_model_config_rejects_llama_embedding_type; model_configs.rs test_user_model_with_llama_embedding_type_is_rejected. The serde error reads: unknown variant `llama-embedding`, expected `ane-embedding`. The loader skips such a file with a warn.
    - resolution.rs platform tests now assert the selected repo, so they still tell the entries apart.
    - commands: cargo test -p swissarmyhammer-config -p swissarmyhammer-embedding --no-fail-fast (all pass; 2 lib tests, test_load_configuration_for_cli_returns_context and test_load_with_cli_args, fail only in a parallel run because of CWD races, pass alone; not related to this card); cargo clippy --all-targets clean; cargo fmt.
    - notes: Linux build not checked (no x86_64 linux gcc here). Remaining "llama-embedding" strings in these crates are the rejection tests and the negative builtin assertion. The nested untracked dir swissarmyhammer/swissarmyhammer holds a stale copy of the repo; not touched. ARCHITECTURE.md still lists the llama-embedding crate; the crate itself is outside this card.
    - next: /review
  timestamp: 2026-11-01T13:43:46.637490+00:00
- actor: claude-code
  id: 01m6bnxyeqn13c59fztnv5yjpq
  text: |-
    ### test — green
    - evidence: cargo nextest run -p config,embedding,kanban,treesitter: 2423 passed, 0 failed, 0 skipped. cargo nextest run -p swissarmyhammer-tools: 961 passed, 0 failed, 0 skipped. cargo clippy --all-targets -D warnings (5 crates): clean. cargo fmt --check: clean. After the config test fix: cargo test -p swissarmyhammer-config --lib 274 passed; nextest -p swissarmyhammer-config 401 passed.
    - flake: the two config tests did not fail in my runs. The cause was real: they read the working directory without serial(cwd), and test_with_template_vars changed the directory to a temp dir and never restored it. Fix (test-only): added serial(cwd) to the two tests, to test_load_configuration_returns_context and to test_load_sets_default_variables; test_with_template_vars now uses CurrentDirGuard. Files: crates/swissarmyhammer-config/src/lib.rs and src/template_context.rs. The clean parent commit was not rebuilt; the race is in code that the embedding change did not touch.
    - next: review
  timestamp: 2026-11-01T13:49:06.391928+00:00
- actor: claude-code
  id: 01m6bnyv8pza7j3xvahtdx8gar
  text: |-
    ### commit — changed
    - evidence: f30a2291d refactor(embedding): remove the llama-embedding backend (19 files: Cargo.lock, swissarmyhammer-embedding, swissarmyhammer-config, task files)
    - next: review
  timestamp: 2026-11-01T13:49:35.894090+00:00
depends_on:
- 01M6B3R61DEYGXGXTTBA0XV2SE
position_column: doing
position_ordinal: '8480'
title: Remove llama-embedding backend from swissarmyhammer-embedding
---
Goal: `Embedder` has one backend, ANE.

Steps:
- In `crates/swissarmyhammer-embedding/Cargo.toml`: remove `llama-embedding`. Make `ane-embedding` a normal dependency, not macOS-only.
- In `src/embedder.rs`: remove the `Llama` variant of `EmbedderBackend`, the `use llama_embedding` line, and the non-macOS paths.
- Remove or rewrite the tests named `from_model_name_llama_embedding_succeeds` and the other llama tests.
- Update `crates/swissarmyhammer-config/src/model.rs` docs and the model config parsing. Reject `type: llama-embedding` with a clear error.
- Update the tests in `crates/swissarmyhammer-config/tests/integration/` that use `llama-embedding` as a fixture type.

Depends on the card "Make every builtin embedding model run on ANE".

Done when: `cargo test -p swissarmyhammer-embedding -p swissarmyhammer-config` passes and no `llama_embedding` reference remains in these crates. #remove-llama-cpp