---
position_column: done
position_ordinal: ffffffffef80
title: BusAddresses missing PartialEq, Eq traits
---
**discovery.rs:12-18**\n\n`BusAddresses` derives `Debug, Clone` but is missing `PartialEq` and `Eq`. It's a public struct with only String fields — these derives are trivially correct and useful for testing and comparison.\n\nPer Rust review guidelines: \"New public types must implement all applicable traits.\"\n\n**Suggestion**: Add `#[derive(Debug, Clone, PartialEq, Eq)]`.\n\n**Verify**: `cargo test -p swissarmyhammer-leader-election` passes.