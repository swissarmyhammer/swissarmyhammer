---
position_column: done
position_ordinal: a2
title: Refactor llama-embedding to implement TextEmbedder trait
---
Update llama-embedding to depend on model-embedding and implement the shared trait. This is a refactor — no new functionality, all existing tests must still pass.

**Changes:**
- `EmbeddingModel` implements `TextEmbedder` trait
- `EmbeddingResult` is now re-exported from model-embedding (not defined locally)
- `BatchProcessor` either becomes generic (from model-embedding) or remains llama-specific but delegates to the generic version
- `EmbeddingConfig` stays llama-specific (contains `ModelSource` from llama-loader, llama-cpp params)
- Error types: llama-embedding errors wrap model-embedding errors + add llama-specific variants (ModelLoader, backend init)

**Key constraint:** The public API should remain backward-compatible. `llama_embedding::EmbeddingResult` still works (via re-export). `EmbeddingModel` still has `new()`, `load_model()`, `embed_text()` methods.

**Checklist:**
- [ ] Add model-embedding dependency to llama-embedding/Cargo.toml
- [ ] Replace local `EmbeddingResult` with re-export from model-embedding
- [ ] Implement `TextEmbedder` for `EmbeddingModel` (map load_model→load, embed_text→embed_text, etc.)
- [ ] Adapt error types to bridge between model-embedding::EmbeddingError and llama-specific errors
- [ ] Update BatchProcessor to use generic or keep llama-specific wrapper
- [ ] All existing unit tests pass unchanged
- [ ] All existing integration tests pass unchanged (real_model_integration.rs)
- [ ] Run tests