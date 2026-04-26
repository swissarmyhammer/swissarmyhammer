---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffc880
title: 'NIT: duration_ms cast truncates to u64 unnecessarily — use u128 or f64'
---
swissarmyhammer-tools/src/mcp/server.rs:1522 and swissarmyhammer-tools/src/mcp/tool_registry.rs:572\n\nBoth logging sites do:\n```rust\nduration_ms = elapsed.as_millis() as u64,\n```\n`Duration::as_millis()` returns `u128`. Casting to `u64` silently truncates for durations over ~584 million years — not a real-world issue — but the `as` cast itself is unnecessary noise since tracing fields accept `u128` directly. The cast also discards sub-millisecond precision that `elapsed.as_millis()` already drops; if sub-ms timing is ever wanted, `elapsed.as_micros()` or `elapsed.as_secs_f64()` would be more idiomatic.\n\nSuggestion: Either use `elapsed.as_millis()` directly (no cast), or use `elapsed.as_secs_f64() * 1000.0` for a float that preserves sub-ms precision. Both are single-character fixes.\n\nVerification: Both occurrences updated consistently; `cargo check` passes."
<parameter name="tags">["review-finding"] #review-finding