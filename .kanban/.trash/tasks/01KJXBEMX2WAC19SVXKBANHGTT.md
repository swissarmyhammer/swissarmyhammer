---
position_column: done
position_ordinal: a9
title: Duplicated BatchProcessor / BatchStats / ProgressInfo across llama-embedding and model-embedding
---
Files:
- llama-embedding/src/batch.rs (entire file)
- model-embedding/src/batch.rs (entire file)

The llama-embedding crate has its own BatchProcessor that is hardcoded to &EmbeddingModel (the concrete llama type), while model-embedding has a generic BatchProcessor<T: TextEmbedder>. This means:

1. BatchStats, ProgressInfo, ProgressCallback, BatchConfig are duplicated nearly character-for-character in both crates.
2. The llama-embedding BatchProcessor does NOT use the model-embedding generic one -- it is a separate, concrete implementation.

Since the whole point of model-embedding is to provide shared abstractions, the llama-embedding batch module should be deleted in favor of re-exporting and using model_embedding::BatchProcessor<EmbeddingModel>.

Suggestion: Remove llama-embedding/src/batch.rs entirely. Have llama-embedding re-export model_embedding::batch types. The llama-specific BatchProcessor adds set_batch_size, get_performance_report, get_model_info -- move those to a trait extension or keep them as methods on a thin wrapper. #review-finding #warning