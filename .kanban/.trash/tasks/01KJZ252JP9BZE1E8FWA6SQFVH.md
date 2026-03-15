---
position_column: done
position_ordinal: i6
title: 'Integration test: verify llama-agent and llama-embedding can still load and use a model end-to-end'
---
**llama-agent/src/model.rs, llama-embedding/src/model.rs**

After the model-loader rename, verify that the full pipeline still works: resolve → download → load into llama-cpp-2 → produce output. Use a small model (e.g., a tiny GGUF embedding model).

**Steps:**
- [ ] Check if integration tests already exist for model loading in llama-agent and llama-embedding
- [ ] Clear the HuggingFace model cache for any test models to force a fresh download (one-time verification that resolve + download still works)
- [ ] Run the integration test that loads a small model and produces embeddings / agent output
- [ ] If no integration test exists, add one (marked `#[ignore]` for CI, runnable manually)
- [ ] Verify the full pipeline: resolve → download → load → inference
- [ ] Document which model is used for testing #review-finding #blocker