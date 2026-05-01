---
attachments: []
depends_on: []
position_column: done
position_ordinal: '8e80'
title: SearchResult missing PartialEq trait impl
---
swissarmyhammer-entity-search/src/result.rs:9-15\n\n`SearchResult` is a public type that derives `Debug, Clone` but not `PartialEq`. Per Rust review guidelines, public types should implement applicable standard traits. `PartialEq` is useful for testing and downstream assertion code. The `f64` score field prevents `Eq` but `PartialEq` works fine.\n\nSuggestion: Add `PartialEq` to the derive list:\n```rust\n#[derive(Debug, Clone, PartialEq)]\npub struct SearchResult { ... }\n```