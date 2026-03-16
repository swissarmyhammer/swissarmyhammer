---
assignees:
- claude-code
position_column: done
position_ordinal: fffff680
title: 'NIT: server.rs span omits caller field present in tool_registry.rs span'
---
swissarmyhammer-tools/src/mcp/server.rs:1493-1498 vs swissarmyhammer-tools/src/mcp/tool_registry.rs\n\nThe two `tool_call` spans are structurally asymmetric:\n\n- `server.rs` span fields: `tool`, `args`, `status` (no `caller` field)\n- `tool_registry.rs` span fields: `tool`, `args`, `caller = \"internal\"`, `status`\n\nThe intent of the `caller` field is to distinguish MCP-protocol calls from internal tool-to-tool calls. Without it on the external call path, consumers of structured logs (e.g., `tracing-subscriber` with JSON output, Jaeger, etc.) cannot easily filter or group by call origin. The asymmetry also makes it harder to define a schema for the `tool_call` span.\n\nSuggestion: Add `caller = \"mcp\"` (or `caller = \"external\"`) to the span in `server.rs::call_tool` to match the field set.\n\nVerification: Both `info_span!` calls contain identical field names; grep for `caller` in both files confirms parity."
<parameter name="tags">["review-finding"] #review-finding