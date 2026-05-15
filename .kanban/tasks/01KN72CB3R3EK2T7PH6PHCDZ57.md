---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8280
title: 'WARNING: ChangeEvent uses stringly-typed JSON payload instead of typed fields'
---
swissarmyhammer-store/src/event.rs:1-15, swissarmyhammer-store/src/handle.rs:697-719\n\nChangeEvent uses `event_name: String` and `payload: serde_json::Value` as its only fields. The payload is constructed via `json!({ \"store\": store_name, \"id\": id })` inside flush_changes() and then parsed back out via `.get(\"store\").and_then(|v| v.as_str())` in the bridge.\n\nThis stringly-typed contract means:\n1. A typo in either the producer or consumer (e.g. \"store_name\" vs \"store\") silently produces empty results\n2. The event_name strings (\"item-created\", \"item-changed\", \"item-removed\") have no compile-time validation\n3. The consumer must do fallible JSON navigation to recover data that was known at construction time\n\nSuggestion: Make ChangeEvent an enum with typed variants:\n```rust\npub enum ChangeEvent {\n    ItemCreated { store_name: String, id: String },\n    ItemChanged { store_name: String, id: String },\n    ItemRemoved { store_name: String, id: String },\n}\n```\nThis eliminates the entire class of bridge parsing bugs and makes the contract compile-time checked. The JSON serialization can happen at the boundary when emitting to the frontend.",
<parameter name="tags">["review-finding"] #review-finding