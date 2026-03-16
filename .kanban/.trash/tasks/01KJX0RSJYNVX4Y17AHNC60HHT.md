---
position_column: done
position_ordinal: a1
title: Create model-embedding crate with shared Embedding trait
---
Extract a shared embedding interface that both llama-embedding and ane-embedding will implement. This is the foundational abstraction.

**Key types to define:**
- `EmbeddingResult` — move from llama-embedding (text, text_hash, embedding vec, sequence_length, processing_time_ms, dimension(), normalize())
- `EmbeddingError` — unified error enum (ModelNotLoaded, TextProcessing, TextEncoding, Configuration, Io) with backend-specific variants
- `BatchConfig` / `BatchStats` / `ProgressInfo` — move batch infrastructure from llama-embedding

**Trait design:**
```rust
#[async_trait]
pub trait TextEmbedder: Send {
    /// Load the model (download if needed, initialize runtime)
    async fn load(&mut self) -> Result<(), EmbeddingError>;
    
    /// Embed a single text
    async fn embed_text(&mut self, text: &str) -> Result<EmbeddingResult, EmbeddingError>;
    
    /// Get embedding dimension (None if not yet loaded)
    fn embedding_dimension(&self) -> Option<usize>;
    
    /// Check if model is ready
    fn is_loaded(&self) -> bool;
}
```

**Checklist:**
- [ ] Create `model-embedding/` crate in workspace
- [ ] Move `EmbeddingResult` from llama-embedding (with md5 hashing, normalize, dimension)
- [ ] Define `EmbeddingError` as a trait-compatible enum (not llama-specific)
- [ ] Define `TextEmbedder` async trait
- [ ] Define `BatchProcessor<T: TextEmbedder>` as a generic over the trait (or move batch to a generic impl)
- [ ] Add `BatchConfig`, `BatchStats`, `ProgressInfo` types
- [ ] Add to workspace Cargo.toml
- [ ] Unit tests for EmbeddingResult, normalization, hashing
- [ ] Run tests