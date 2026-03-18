---
position_column: done
position_ordinal: ffffffffeb80
title: ElectionError::Message loses error chain — use source
---
**error.rs:25-26**\n\n`ElectionError::Message(String)` wraps error messages as strings, discarding the original error's `source()` chain. Used by `HebEvent::to_frames` and `from_frames` for serde errors, and by Publisher/Subscriber for channel errors.\n\nPer Rust review guidelines: \"Error::source() chains must exist for wrapped errors — don't flatten the chain.\"\n\n**Suggestion**: Replace with a `Serialization(Box<dyn std::error::Error + Send + Sync>)` variant that preserves the source, or add separate variants for each underlying error type.\n\n**Verify**: `cargo test -p swissarmyhammer-leader-election -p heb` passes.