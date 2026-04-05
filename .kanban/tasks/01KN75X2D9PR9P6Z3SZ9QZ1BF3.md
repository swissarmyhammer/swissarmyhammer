---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9880
title: Test embedding Embedder construction and TextEmbedder delegation
---
File: swissarmyhammer-embedding/src/embedder.rs (17.7% coverage, 31/175 lines)\n\nUncovered functions:\n- Embedder::default() and from_model_name() - model resolution and backend selection (lines 57-133)\n- backend_name(), model_name(), max_sequence_length() - accessors (lines 136-152)\n- TextEmbedder impl: load(), embed_text(), embedding_dimension(), is_loaded() (lines 160-195)\n- embed_single() and embed_chunked() - single vs chunked embedding with mean-pooling (lines 197-280)\n\nThis is the main embedding facade. Testing from_model_name() requires either a real model on disk or mocking ModelManager. The chunk-and-pool logic in embed_chunked() is independently testable." #coverage-gap