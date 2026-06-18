---
assignees:
- claude-code
position_column: todo
position_ordinal: b180
project: diagnostics
title: Extract shared op-tool dispatch helpers (json_result, string_arg, …) across review/code_context/diagnostics
---
## What
`json_result` (serialize a value into a JSON-text `CallToolResult`) and the small arg readers (`string_arg`, `bool_arg`, `string_array_arg`) are now duplicated identically across at least three op-dispatched MCP tools:
- `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs`
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs`

All three `json_result`s are byte-identical (Result<CallToolResult, rmcp::ErrorData>, since McpError = rmcp::ErrorData). Rule-of-three reached → extract to one shared module and have every op-tool import it.

## Why deferred (from na6cvh0 review)
Surfaced by the na6cvh0 review. Not folded into na6cvh0 because a correct consolidation must touch code_context/mod.rs, which had unrelated uncommitted WIP at the time; doing it as its own task keeps that change clean and reviewable.

## Plan
- Add a shared module (e.g. `crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs` or under `tool_registry`) with `json_result`, `string_arg`, `bool_arg`, `string_array_arg`.
- Replace the per-tool copies in review, code_context, and diagnostics with imports.
- `cargo build --workspace` + the tools test suite as the guard.

## Acceptance Criteria
- [ ] One canonical `json_result` + arg readers; review/code_context/diagnostics all import them (no per-tool copies).
- [ ] tools crate builds + tests green; no behavior change.

#diagnostics