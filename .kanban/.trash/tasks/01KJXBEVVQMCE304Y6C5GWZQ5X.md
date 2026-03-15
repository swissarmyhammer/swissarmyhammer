---
position_column: done
position_ordinal: b0
title: 'llama-embedding: EmbeddingResult type collision from dual definitions'
---
Files:
- llama-embedding/src/types.rs:5 -- re-exports `model_embedding::EmbeddingResult`
- llama-embedding/src/lib.rs:73-74 -- re-exports `error::EmbeddingResult as Result` AND `types::EmbeddingResult`
- llama-embedding/src/batch.rs:1-3 -- imports both `error::EmbeddingResult as Result` and `types::EmbeddingResult`

The name `EmbeddingResult` is used for two completely different things:
1. `model_embedding::EmbeddingResult` -- the embedding output struct
2. `error::EmbeddingResult<T>` -- a Result type alias `Result<T, EmbeddingError>`

In lib.rs line 73-74:
```rust
pub use error::{EmbeddingError, EmbeddingResult as Result};
pub use types::{EmbeddingConfig, EmbeddingResult};
```

This is confusing for downstream users. `use llama_embedding::EmbeddingResult` gives the struct, but the crate also re-exports a `Result` alias. This works but is a naming footgun.

Suggestion: Rename the error type alias to avoid the collision. Consider `pub type EmbeddingResult<T> = ...` -> `pub type EmbeddingOutcome<T> = ...` or just use `Result<T>` pattern consistently without re-exporting the alias from the crate root. #review-finding #warning