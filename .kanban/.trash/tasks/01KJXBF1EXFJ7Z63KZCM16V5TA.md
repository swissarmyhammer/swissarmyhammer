---
position_column: done
position_ordinal: b1
title: 'llama-embedding: EmbeddingModel::embed_text returns model_embedding::EmbeddingResult but batch.rs expects llama_embedding types'
---
File: llama-embedding/src/batch.rs:247-249

The TextEmbedder trait returns `model_embedding::EmbeddingResult`. But the batch.rs error branch on line 258 does `EmbeddingError::text_processing(e.to_string())` -- wrapping the model_embedding error into a llama_embedding error by stringifying it, losing the original error chain.

This violates the Rust review guideline: "Error::source() chains must exist for wrapped errors -- don't flatten the chain."

Suggestion: Instead of `EmbeddingError::text_processing(e.to_string())`, add a variant like `EmbeddingError::SharedEmbedding(model_embedding::EmbeddingError)` or use the Backend wrapper pattern already in the trait impl. #review-finding #warning