---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffb480
title: 'heb/store.rs: ZMQ send failure silently swallowed in HebContext::publish()'
---
heb/src/context.rs:62

```rust
let _ = self.election.publish(&event);
```

The ZMQ publish result is discarded. This is documented as "best-effort" but there is no tracing log on failure and no way for the caller to distinguish "ZMQ failed but SQLite succeeded" from "both succeeded". This makes diagnosing live delivery problems very hard.

Suggestion: at minimum `tracing::warn!` on ZMQ send error. The function signature could also return `(u64, Option<ElectionError>)` to let callers act on partial success if needed.