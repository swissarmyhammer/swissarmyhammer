---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd180
title: Test llama-embedding LlamaModel load/embed lifecycle
---
File: llama-embedding/src/model.rs (0% coverage, 0/109 lines)\n\nEntirely untested. Key functions:\n- LlamaModel::new() - creates model from config path\n- TextEmbedder::load() - loads GGUF model into memory\n- TextEmbedder::embed_text() - tokenize + forward pass + extract embedding\n- embedding_dimension(), is_loaded()\n\nRequires a real GGUF model file on disk. Consider a small test model or #[ignore] tests that run only when the model is available. At minimum, test error paths (missing model file, invalid path)." #coverage-gap