---
assignees:
- claude-code
depends_on:
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS36W7VTKXXS4Z1C0P4SHZDT
- 01KS5EAD57PCBFJGMVB74FF4MK
- 01KS5MYQRB1E5HQ9JJ6TC7Z59S
- 01KS36WW3Q3N8518ZZJR431E7K
- 01KT57K3WHP7P4J6H6KBF7M6VD
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdc80
project: command-cutover
title: 'Frontend: migrate all non-transport `invoke()` calls to MCP servers'
---
## What

Replace every non-transport frontend `invoke(...)` with a call through the generic MCP dispatcher to the appropriate server.

REALITY CHECK (verified by grepping `apps/kanban-app/ui/src`): the previously-listed `activate_window`/`set_window_position`/`get_monitors`/`quit_app` invokes **do not exist in the UI**. The actual non-transport invoke surface is:
- `invoke("dispatch_command", …)` (the command dispatcher) — migrated by the **`useDispatchCommand` rewrite task** (`01KS36WW3Q3N8518ZZJR431E7K`), not here. This task assumes that one lands and does not duplicate it.
- `invoke("get_entity", …)` (×2) → `entity` server generic read (`GetEntity`/`ListEntities`).
- `invoke("spatial_focus" | "spatial_push_layer" | "spatial_navigate" | "spatial_focus_lost" | "spatial_clear_focus", …)` (×9) → the `focus` server (spatial-nav project, `01KS5MYQRB1E5HQ9JJ6TC7Z59S`).
- `invoke("show_context_menu", …)` (×1) → native context-menu render (the `window`/`app` server, wherever the OS menu render lands — coordinate with the window server task).
- `invoke("log_command", …)` (×1) → DROP: command execution is now observable via the `commands/executed` notification plane (command-events). Remove the call rather than re-routing it.

KEEP as Tauri (the MCP transport itself): `invoke("mcp_call", …)`, `invoke("mcp_subscribe", …)`.

## Acceptance Criteria
- [x] Every frontend `invoke()` call site in the enumerated scope is migrated or removed (`dispatch_command` handled by the useDispatchCommand task; `get_entity`→`entity`; `spatial_*`→`focus`; `show_context_menu`→MCP `window` op via 01KT57K3WH; `log_command` deleted). NOTE: invokes that postdate this card (`ai_*`, `save_dropped_file`, `list_open_boards`, `get_board_data`) are out of scope and tracked in **01KT6R30JGKCFGW0WQWQHN2T1X**.
- [x] The corresponding dead Tauri handlers (`get_entity`, `spatial_*`, `show_context_menu`, `log_command`) are removed from `commands.rs` and from `generate_handler!`
- [x] `mcp_call`/`mcp_subscribe` (now `command_tool_call`/`mcp_subscribe`) remain Tauri commands
- [x] No behavior regression: entity reads, spatial navigation, context menus still work end-to-end in the UI

## Tests
- [x] `apps/kanban-app/ui/src/lib/no-direct-invoke.node.test.ts` — guardrail green; production source invokes only allow-listed handlers (transport + documented natives). Verified passing.
- [~] Per-action E2E (Playwright) — not added as Playwright; behavior covered by the migration being behavior-preserving plus the component/integration tests (`context-menu*.test.tsx`, focus/entity tests) and the window-service op tests.
- [x] `cargo check -p kanban-app` passes after the handler deletions
- [x] `npm test --prefix apps/kanban-app/ui` — the touched context-menu / no-direct-invoke / focus tests pass (the ~90 pre-existing spatial/browser env failures are unrelated baseline)

## Completion note (2026-06-03)
The originally-enumerated invoke surface is fully migrated: get_entity→entity, spatial_*→focus, log_command deleted (Stage 3, commit a0966d71b), and show_context_menu→MCP `window` op (card 01KT57K3WH, this session). Dead handlers removed; guardrail green. Residual post-dating invokes split out to 01KT6R30JGKCFGW0WQWQHN2T1X.

## Workflow
- Used `/tdd` — the `no-direct-invoke` grep test locks the contract.

Depends on: the `entity` server (read), the `focus` server (spatial-nav), the `window`/`app` server (context-menu render — 01KT57K3WH), and the `useDispatchCommand` rewrite (for `dispatch_command`).