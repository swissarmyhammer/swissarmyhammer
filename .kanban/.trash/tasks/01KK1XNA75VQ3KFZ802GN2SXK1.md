---
position_column: todo
position_ordinal: g5
title: Update config tests for multi-executor format
---
Update and add tests for the new multi-executor YAML format:

- [ ] Update all tests in `swissarmyhammer-config/tests/integration/model_configs.rs` that construct `ModelConfig { executor: ..., quiet: ... }` to use new `executors` vec or convenience constructors
- [ ] Update `swissarmyhammer-config/tests/integration/agent_config_file_loadings.rs` — YAML/TOML parsing tests
- [ ] Update `swissarmyhammer-config/tests/integration/llama_config.rs`
- [ ] Update `swissarmyhammer-config/tests/integration/agent_hierarchical_configs.rs`
- [ ] Add test: parse old `executor:` format (singular) still works — backward compat
- [ ] Add test: parse new `executors:` format (list) works
- [ ] Add test: `select_executor()` picks ANE on macos-arm64, llama as fallback
- [ ] Add test: `select_executor()` skips platform-constrained entries that don't match
- [ ] Add test: round-trip serialization of multi-executor config
- [ ] Add test: `AneEmbedding` variant serializes/deserializes correctly
- [ ] Run full test suite

Files: `swissarmyhammer-config/tests/integration/`, `swissarmyhammer-config/src/model.rs` (unit tests)