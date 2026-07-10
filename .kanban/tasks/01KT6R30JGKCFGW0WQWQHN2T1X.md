---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe680
project: command-cutover
title: Audit + migrate remaining non-transport Tauri invokes to MCP (ai_*, board, file-drop)
---
DISCOVERED finishing 01KS36Y4 (frontend invoke migration). That card's REALITY-CHECK enumerated the invoke surface as of its writing (get_entity/spatial_*/show_context_menu/log_command/dispatch_command) — all now migrated, guardrail green. But the `no-direct-invoke` allow-list has since grown with invokes from features that postdate that card. Per "everything is MCP", audit each and migrate the ones that should be MCP.

## Current non-transport allow-listed invokes (apps/kanban-app/ui/src/lib/no-direct-invoke.node.test.ts ALLOWED_INVOKE_HANDLERS)
Transport (KEEP, the seam): `command_tool_call`, `mcp_subscribe`. Also currently kept: `dispatch_command` (legacy unified dispatcher used internally by `mcp-transport.ts::callCommandTool` for the `execute command` verb — keep until/unless that routing changes).

To assess/migrate:
- `ai_set_streaming` (ai-panel-container.tsx:398), `ai_start_agent`, `ai_list_models` — AI panel / in-process agent registry. May warrant an `ai`/agent MCP server, OR legitimately stay native (WebSocket bridge spawn). Decide per op. (ai-panel project territory.)
- `save_dropped_file` — HTML5-drop bytes → temp file → attachment path. Candidate for a `window`/files MCP op (OS file write).
- `list_open_boards`, `get_board_data` — board management reads. Candidates for `window`/`entity` MCP ops.

## Work
- For each invoke above: decide MCP op vs legitimately-native (document the call), and migrate the ones that should be MCP (add the op on the appropriate server, route the frontend through the transport, remove the Tauri handler + allow-list entry).
- Update the allow-list so it contains ONLY genuine transport + documented-native exceptions, each with a one-line justification.

## Acceptance
- Every remaining allow-listed invoke is either migrated to MCP (handler removed) or carries a written justification for staying native.
- `no-direct-invoke` guardrail green; `cargo check -p kanban-app` + UI tests green.

Note: this is the residual of the big-bang cutover's frontend-invoke goal; the originally-enumerated set is done (01KS36Y4).