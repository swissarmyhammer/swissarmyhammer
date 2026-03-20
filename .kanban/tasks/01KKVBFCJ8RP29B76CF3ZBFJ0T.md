---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9380
title: Validator tool_context leaks full registry to validator tools
---
**swissarmyhammer-tools/src/mcp/server.rs:create_validator_server()**\n\nThe validator McpServer clones `tool_context` from the parent server. That context's `tool_registry` field still points to the parent's full (unfiltered) `Arc<RwLock<ToolRegistry>>`. If a validator tool calls `context.call_tool(\"kanban\", ...)`, it succeeds — bypassing the lockdown.\n\n**Why this matters:** The entire point of the validator endpoint is security lockdown. A validator prompt could instruct the LLM to call `context.call_tool()` with any tool name.\n\n**Fix:** In `create_validator_server()`, replace the cloned context's `tool_registry` with the validator-only registry:\n```rust\nlet validator_registry_arc = Arc::new(RwLock::new(validator_registry));\nlet mut validator_context = (*self.tool_context).clone();\nvalidator_context.tool_registry = Some(validator_registry_arc.clone());\n```\n\n**Verification:** Write a test that creates a validator server and confirms `call_tool(\"kanban\", ...)` fails.</description>
<parameter name="tags">["review-finding"]