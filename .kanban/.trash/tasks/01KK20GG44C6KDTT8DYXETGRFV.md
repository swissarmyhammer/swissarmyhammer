---
position_column: todo
position_ordinal: c3
title: Migrate treesitter index to swissarmyhammer-embedding
---
Replace direct `llama_embedding` usage in `swissarmyhammer-treesitter/src/index.rs` with `swissarmyhammer_embedding::Embedder::from_model_name(...)`. This will automatically select ANE on Apple Silicon and llama.cpp elsewhere.