---
position_column: todo
position_ordinal: b9
title: Unify hardcoded test model constants into builtin model configs
---
Replace scattered hardcoded test model references with named builtin model configs. Currently there are conflicting constants:

**Embedding test models (3 different values!):**
- `swissarmyhammer-config/src/lib.rs:391` — `DEFAULT_TEST_EMBEDDING_MODEL = "BAAI/bge-small-en-v1.5"`
- `swissarmyhammer-common/src/constants.rs:29` — `DEFAULT_TEST_EMBEDDING_MODEL = "nomic-ai/nomic-embed-code"`
- `llama-agent/src/test_models.rs:59` — `TEST_EMBEDDING_MODEL_REPO = "Qwen/Qwen3-Embedding-0.6B-GGUF"`

**LLM test models (duplicated):**
- `swissarmyhammer-config/src/lib.rs:370` — `DEFAULT_TEST_LLM_MODEL_REPO = "unsloth/Qwen3-0.6B-GGUF"`
- `llama-agent/src/test_models.rs:42` — `TEST_MODEL_REPO = "unsloth/Qwen3-0.6B-GGUF"` (same)
- `llama-agent/src/tests/test_utils.rs:273` — hardcoded `"unsloth/Qwen3-1.7B-GGUF"` for compaction tests
- `builtin/models/qwen-0.6b-test.yaml` — already a builtin config

Steps:
- [ ] Decide canonical test embedding model (recommend: `qwen-embedding` builtin, since it already exists and is the smallest)
- [ ] Update `swissarmyhammer-config` and `swissarmyhammer-common` test constants to point at the `qwen-embedding` builtin model name (or resolve the conflict)
- [ ] Have `TestConfig::from_environment()` load from builtin model configs by name instead of raw repo strings
- [ ] Update `llama-agent/src/test_models.rs` to reference builtin model names
- [ ] Convert `qwen-0.6b-test.yaml` to multi-executor format (add ANE variant for Apple Silicon test runs)
- [ ] Consider adding a `qwen-embedding-test.yaml` or just reuse `qwen-embedding.yaml` (it's already 0.6B, small enough for testing)
- [ ] Remove duplicate constants, centralize in one place
- [ ] Run all tests to verify nothing breaks

Files: `llama-agent/src/test_models.rs`, `swissarmyhammer-config/src/lib.rs`, `swissarmyhammer-common/src/constants.rs`, `llama-agent/src/tests/test_utils.rs`, `builtin/models/qwen-0.6b-test.yaml`