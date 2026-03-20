---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9a80
title: ValidatorTool marker trait is unused for dispatch
---
**swissarmyhammer-tools/src/mcp/tool_registry.rs:ValidatorTool trait**\n\nThe `ValidatorTool` marker trait is implemented on `CodeContextTool` and `FilesTool`, but nothing checks `impl ValidatorTool` at compile time or runtime. The actual filtering uses `is_validator_tool()` — a method on the `McpTool` trait. The marker trait exists only for documentation symmetry with `AgentTool`.\n\n**Why this matters (nit):** The `AgentTool` trait also isn't checked at compile time, so this is consistent with the existing pattern. However, both patterns could be stronger — a function like `register_as_validator<T: ValidatorTool>(registry, tool)` would enforce the constraint at compile time.\n\n**Fix:** No action required — this is consistent with the existing `AgentTool` pattern. Consider adding a compile-time check in a future cleanup.\n\n**Verification:** N/A