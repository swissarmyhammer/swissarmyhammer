---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: ai-panel
title: Extract kanban MCP tool schema and error classifier into swissarmyhammer-kanban
---
## What
The kanban MCP stdio server in `apps/kanban-cli/src/commands/serve.rs` defines the `kanban` tool shape (`build_list_tools_result`, tool name/description constants, schema generation) and the `KanbanError` -> `rmcp::ErrorData` classifier (`classify_kanban_error`, `classify_kanban_error_kind`, `classify_entity_error_kind`). Phase 1 of the AI panel adds a SECOND MCP server (HTTP, in the Tauri app) that must expose the identical tool and identical error mapping. Lift the shared pieces into `swissarmyhammer-kanban` so the two servers cannot drift.

- Create `crates/swissarmyhammer-kanban/src/mcp.rs` holding: tool name/description constants, `build_list_tools_result()`, and the three error-classification functions.
- Re-export the public surface from `crates/swissarmyhammer-kanban/src/lib.rs`.
- Update `apps/kanban-cli/src/commands/serve.rs` to call the extracted functions; delete the duplicated bodies. `serve.rs` keeps only the stdio-transport wiring.
- Add `rmcp` to `swissarmyhammer-kanban/Cargo.toml` if not already present (it is a workspace dep).

Spec: `ideas/kanban/ai_panel.md` — Phase 1, "What Phase 1 delivers".

## Acceptance Criteria
- [ ] `swissarmyhammer-kanban` publicly exposes the kanban tool-schema builder and the `KanbanError` -> `McpError` classifier.
- [ ] `apps/kanban-cli/src/commands/serve.rs` contains no duplicated schema or classifier code — it calls the extracted functions.
- [ ] `cargo build -p swissarmyhammer-kanban -p kanban-cli` is clean with no new warnings.

## Tests
- [ ] Port the existing `serve.rs` classifier unit tests (`classify_kanban_error_maps_caller_input_failures_to_invalid_params`, `..._state_conflicts_to_invalid_request`, `..._server_failures_to_internal_error`, the `EntityError` variants, `..._prepends_context_to_message`) into `crates/swissarmyhammer-kanban/src/mcp.rs` and keep them passing.
- [ ] Keep `serve.rs` tests `list_tools_returns_single_kanban_tool` and the `call_tool_*` tests green against the extracted code.
- [ ] `cargo test -p swissarmyhammer-kanban -p kanban-cli` is green.

## Workflow
- Use `/tdd` — port the tests into their new home first, watch them fail, then move the implementations to make them pass.