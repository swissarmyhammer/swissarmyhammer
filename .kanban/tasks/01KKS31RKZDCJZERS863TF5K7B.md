---
assignees:
- claude-code
position_column: done
position_ordinal: fffff880
title: 'NIT: tool_registry.rs logs tool name redundantly in both the span and the event'
---
swissarmyhammer-tools/src/mcp/tool_registry.rs (tool_call complete event)\n\nThe span already carries `tool = %tool_name` as a field. The completion event at the bottom of both call sites then re-logs `tool = %tool_name` as a separate field on the `info!` event:\n```rust\ntracing::info!(\n    tool = %tool_name,\n    duration_ms = elapsed.as_millis() as u64,\n    error = is_error,\n    \"tool_call complete\"\n);\n```\nIn tracing-subscriber's default formatters (and in JSON exporters) the span's `tool` field is already attached to every event emitted inside that span. Adding it again to the event body creates a duplicate key in JSON output, which can confuse log processors.\n\nSuggestion: Remove `tool = %tool_name` from the `info!` event in both `server.rs` and `tool_registry.rs`; the span context carries it.\n\nVerification: Check JSON log output for duplicate `tool` keys."
<parameter name="tags">["review-finding"] #review-finding