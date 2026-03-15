---
position_column: todo
position_ordinal: b7
title: Convert models to CoreML and upload to HuggingFace
---
Run the generalized converter for all three models and upload to HuggingFace under `wballard/`:

- [ ] Convert Qwen3-Embedding-0.6B to CoreML with palettize4 quantization
- [ ] Convert nomic-embed-code-v2 to CoreML with palettize4 quantization
- [ ] Convert jina-embeddings-v3 to CoreML with palettize4 quantization (after LoRA merge)
- [ ] Verify each .mlpackage loads and produces correct-dimension embeddings
- [ ] Create HuggingFace repos:
  - `wballard/Qwen3-Embedding-0.6B-CoreML`
  - `wballard/nomic-embed-code-v2-CoreML`
  - `wballard/jina-embeddings-v3-CoreML`
- [ ] Upload .mlpackage + tokenizer.json to each repo
- [ ] Add model cards with metadata (architecture, dimensions, seq length, quantization)
- [ ] Verify download works via `huggingface-cli download`

Depends on: converter script generalization