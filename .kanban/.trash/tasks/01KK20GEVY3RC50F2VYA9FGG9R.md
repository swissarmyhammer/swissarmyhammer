---
position_column: todo
position_ordinal: c2
title: Migrate shell semantic search to swissarmyhammer-embedding
---
Replace direct `llama_embedding` usage in `swissarmyhammer-tools/src/mcp/tools/shell/state.rs` with `swissarmyhammer_embedding::Embedder::from_model_name(...)`. This will automatically select ANE on Apple Silicon and llama.cpp elsewhere.