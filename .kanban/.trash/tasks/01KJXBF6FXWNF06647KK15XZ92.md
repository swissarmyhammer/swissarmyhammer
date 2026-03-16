---
position_column: done
position_ordinal: b2
title: 'model-embedding: TextEmbedder trait should be sealed'
---
File: model-embedding/src/lib.rs:16-31

Per Rust review guidelines: "Sealed traits for public traits not meant to be implemented downstream. Prevents semver hazards when adding methods."

The TextEmbedder trait is public and intended to be implemented only by crates in this workspace (llama-embedding, future ane-embedding). If a downstream crate implements it, adding a new method to the trait is a breaking change.

Suggestion: Add a sealed pattern using a private supertrait:
```rust
mod private { pub trait Sealed {} }
pub trait TextEmbedder: private::Sealed + Send + Sync { ... }
```
Then impl Sealed for each known backend. #review-finding #warning