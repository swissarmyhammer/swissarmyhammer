---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8e80
title: 'WARNING: Bridge pattern silently drops events with empty store_name or id'
---
kanban-app/src/commands.rs:1433-1436\n\nThe ChangeEvent-to-WatchEvent bridge silently skips events where store_name or id is empty:\n\n```rust\nlet store_name = se.payload.get(\"store\").and_then(|v| v.as_str()).unwrap_or(\"\");\nlet id = se.payload.get(\"id\").and_then(|v| v.as_str()).unwrap_or(\"\");\nif store_name.is_empty() || id.is_empty() {\n    continue;\n}\n```\n\nThis is a correctness risk. If a TrackedStore implementation returns a default store_name() of \"unknown\" (because the directory has no basename -- e.g. root \"/\"), the event passes through but with the wrong entity_type. If the payload is malformed (missing \"store\" or \"id\" keys), the event is silently swallowed with no log message.\n\nSuggestion: Add a tracing::warn when skipping events so malformed payloads are observable in logs. Also consider making the bridge use typed fields on ChangeEvent rather than parsing JSON string payloads, which would make this class of bug impossible at compile time.",
<parameter name="tags">["review-finding"] #review-finding