---
position_column: todo
position_ordinal: b6
title: Generalize CoreML converter for multiple embedding models
---
Generalize `ane-embedding/convert/convert_qwen_embedding.py` into a reusable converter that handles Qwen, nomic, and jina models:

- [ ] Create `ane-embedding/convert/convert_embedding.py` — generic converter that takes `--model` arg
- [ ] Extract `EmbeddingWithPooling` wrapper class (already generic — mean pooling over last_hidden_state)
- [ ] Model registry dict mapping model names to HuggingFace repos and expected hidden dims:
  - `qwen-embedding`: `Qwen/Qwen3-Embedding-0.6B` (1024d)
  - `nomic-embed-code`: `nomic-ai/nomic-embed-code-v2` (768d) — BERT-class, standard conversion
  - `jina-embeddings-v3`: `jinaai/jina-embeddings-v3` — XLM-RoBERTa + LoRA adapters, need to merge LoRA weights before export
- [ ] Handle jina's LoRA adapter merging: load base model, apply adapters, then wrap with pooling
- [ ] Parameterize output directory per model
- [ ] Keep existing quantization options (palettize4 recommended)
- [ ] Verify each model with dummy inference after conversion
- [ ] Save tokenizer alongside each .mlpackage
- [ ] Keep old `convert_qwen_embedding.py` as a thin wrapper that calls the generic converter with `--model qwen-embedding`
- [ ] Test all three conversions locally

Files: `ane-embedding/convert/convert_embedding.py` (new), `ane-embedding/convert/convert_qwen_embedding.py` (update)