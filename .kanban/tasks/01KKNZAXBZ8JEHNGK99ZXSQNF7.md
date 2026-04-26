---
position_column: done
position_ordinal: '9180'
title: Unused async-trait dependency in swissarmyhammer-entity-search
---
swissarmyhammer-entity-search/Cargo.toml:15\n\n`async-trait = { workspace = true }` is listed as a dependency but never used in any source file. The crate uses `async fn` in trait impl positions (which is stable since Rust 1.75) rather than the `#[async_trait]` macro.\n\nSuggestion: Remove `async-trait` from `[dependencies]`.