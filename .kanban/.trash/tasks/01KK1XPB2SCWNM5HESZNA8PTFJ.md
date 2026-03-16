---
position_column: todo
position_ordinal: b8
title: Create multi-executor builtin model YAML configs
---
Create/update builtin model configs with both ANE and llama executors:

- [ ] Update `builtin/models/qwen-embedding.yaml` — add `executors` list with `ane-embedding` (platform: macos-arm64, repo: wballard/Qwen3-Embedding-0.6B-CoreML) and `llama-embedding` (universal fallback, existing GGUF repo)
- [ ] Create `builtin/models/nomic-embed-code.yaml` — `executors` list with `ane-embedding` (platform: macos-arm64, repo: wballard/nomic-embed-code-v2-CoreML) and `llama-embedding` (repo: nomic-ai/nomic-embed-code-v2-GGUF or equivalent)
- [ ] Create `builtin/models/jina-embeddings-v3.yaml` — `executors` list with `ane-embedding` (platform: macos-arm64, repo: wballard/jina-embeddings-v3-CoreML) and `llama-embedding` (repo: jinaai/jina-embeddings-v3-GGUF or equivalent)
- [ ] Verify GGUF repos exist on HuggingFace for nomic and jina (find correct repo names and filenames)
- [ ] Ensure all three configs parse correctly with `parse_model_config()`
- [ ] Update treesitter `EmbeddingModelConfig::default()` to reference `nomic-embed-code` builtin model instead of hardcoded repo string

Depends on: config format changes + HuggingFace uploads

Files: `builtin/models/qwen-embedding.yaml`, `builtin/models/nomic-embed-code.yaml` (new), `builtin/models/jina-embeddings-v3.yaml` (new), `swissarmyhammer-treesitter/src/index.rs`