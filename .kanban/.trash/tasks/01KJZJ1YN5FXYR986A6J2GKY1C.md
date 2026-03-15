---
position_column: done
position_ordinal: m8
title: 'model-embedding: BatchProcessor error type loses backend-specific context'
---
**File:** model-embedding/src/batch.rs:221-236\n\n**What:** `BatchProcessor::process_batch()` calls `self.model.embed_text(text).await` which returns `model_embedding::EmbeddingError`. When `continue_on_error` is true, failures are logged with `warn!` but the error details are lost -- only a count is tracked in stats. When `continue_on_error` is false, the raw error propagates, which is fine.\n\n**Why:** For batch operations where partial failure is expected, callers have no way to inspect which texts failed and why. The `BatchStats` only tracks counts, not the actual errors.\n\n**Suggestion:** Consider collecting failed items as `Vec<(usize, String, EmbeddingError)>` (index, text preview, error) in `BatchStats` or as a separate return value, so callers can decide how to handle partial failures. #review-finding #warning