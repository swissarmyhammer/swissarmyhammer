---
position_column: done
position_ordinal: a5
title: Create ane-embedding crate implementing TextEmbedder
---
Build the ANE embedding crate that implements the same `TextEmbedder` trait as llama-embedding, but uses ONNX Runtime + CoreML for Apple Neural Engine inference.

**Architecture (from ideas/ane-embed-crate-plan.md):**
- Uses `ort` crate with CoreML execution provider
- Uses `tokenizers` crate for HuggingFace tokenizer handling
- Uses `model-loader` for HuggingFace download / local file resolution (same as llama-embedding)
- Implements `TextEmbedder` from model-embedding crate

**Key differences from llama-embedding:**
- Runtime: ONNX Runtime (via `ort`) instead of llama-cpp-2
- Model format: ONNX instead of GGUF
- Tokenization: `tokenizers` crate instead of llama-cpp built-in tokenizer
- Compute: CoreML EP → ANE on Apple Silicon, CPU fallback elsewhere
- Static input shapes: ANE prefers fixed-size inputs, so pad/truncate to model max length
- Pooling: mean pooling / CLS token (ONNX embedding models need explicit pooling, unlike llama-cpp which handles it internally)

**Config type (ane-specific):**
```rust
pub struct AneEmbeddingConfig {
    pub model_source: ModelSource,        // from model-loader
    pub normalize_embeddings: bool,
    pub max_sequence_length: Option<usize>,
    pub pooling: Pooling,                 // Mean, CLS, LastToken
    pub debug: bool,
}
```

**Session builder with auto-detection:**
```rust
// On macOS aarch64: CoreML EP with CPUAndNeuralEngine
// Elsewhere: CPU fallback
```

**Model registry — known-good ONNX models:**
- all-MiniLM-L6-v2 (22M, 384-dim)
- BGE-small-en-v1.5 (33M, 384-dim)  
- Nomic-embed-text-v1.5 (137M, 768-dim)

**Checklist:**
- [ ] Create `ane-embedding/` crate in workspace
- [ ] Add deps: ort, tokenizers, model-embedding, model-loader, ndarray
- [ ] Implement `AneEmbeddingModel` struct
- [ ] Implement `TextEmbedder` trait for `AneEmbeddingModel`
- [ ] Implement tokenization with static shape padding
- [ ] Implement mean pooling + L2 normalization
- [ ] Implement CoreML EP auto-detection in session builder
- [ ] Unit tests mirroring llama-embedding test patterns
- [ ] Integration tests with real ONNX model (all-MiniLM-L6-v2 is small/fast)
- [ ] Run tests