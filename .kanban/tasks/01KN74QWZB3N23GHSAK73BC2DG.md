---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffe80
title: Set _meta['anthropic/alwaysLoad'] on all sah MCP tools
---
## What

MCP tools from the sah server are currently deferred by Claude Code because they lack the `_meta` annotation that signals eager loading. Set `_meta: { "anthropic/alwaysLoad": true }` on every tool definition so they are always available without requiring a `ToolSearch` round-trip.

**File to modify:** `swissarmyhammer-tools/src/mcp/tool_registry.rs` — the `list_tools()` method (around line 1103-1119) constructs `rmcp::model::Tool` instances via `Tool::new(...).with_title(...)`. None currently set the `meta` field.

**The `Tool` struct** (from `rmcp` v1.2.0) has `pub meta: Option<Meta>` (serialized as `_meta`). `Meta` wraps `serde_json::Map<String, Value>`, so arbitrary keys can be inserted.

**Approach:** After constructing each `Tool` in `list_tools()`, set its `meta` field:
```rust
let mut meta_map = serde_json::Map::new();
meta_map.insert("anthropic/alwaysLoad".into(), serde_json::Value::Bool(true));
tool.meta = Some(rmcp::model::Meta(meta_map));
```

This is a single modification point — all tools flow through `list_tools()`.

**Tools affected:** git, kanban, questions, web, code_context, shell, ralph, agent, file (read/write/edit/grep/glob), skill — all tools registered in `server.rs` lines 768-777.

## Acceptance Criteria
- [x] Every tool returned by `list_tools()` has `_meta: { "anthropic/alwaysLoad": true }` in its JSON representation
- [x] Claude Code no longer defers sah tools (they appear in the tool list without requiring ToolSearch)
- [x] Existing tool functionality is unchanged (names, schemas, descriptions)

## Tests
- [x] Add unit test in `swissarmyhammer-tools/src/mcp/tool_registry.rs` that calls `list_tools()` and asserts every `Tool` has `meta` containing `"anthropic/alwaysLoad": true`
- [x] `cargo test -p swissarmyhammer-tools` passes