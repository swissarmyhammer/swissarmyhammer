---
assignees:
- claude-code
depends_on:
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS36W7VTKXXS4Z1C0P4SHZDT
- 01KS5EAD57PCBFJGMVB74FF4MK
- 01KS5MYQRB1E5HQ9JJ6TC7Z59S
- 01KS36WW3Q3N8518ZZJR431E7K
position_column: todo
position_ordinal: '9280'
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

Files (in `apps/kanban-app/ui/src/**/*.{ts,tsx}`): migrate each call site above to `dispatcher.<server>.<tool>.<verb>({…})`. Then delete the now-dead Rust handlers and remove them from `tauri::Builder::invoke_handler(generate_handler![...])`:
- `get_entity` handler (`apps/kanban-app/src/commands.rs:352`, registered `main.rs:71`) — removed once the `entity` server read path replaces it.
- `spatial_*` handlers (`apps/kanban-app/src/commands.rs:2188+`) — removed once `focus` server lands.
- `show_context_menu`, `log_command` handlers — removed/replaced.
- `mcp_call`/`mcp_subscribe` handlers — KEEP.

## Acceptance Criteria
- [ ] Every frontend `invoke()` call site except `mcp_call`/`mcp_subscribe` is migrated or removed (`dispatch_command` handled by the useDispatchCommand task; `get_entity`→`entity`; `spatial_*`→`focus`; `show_context_menu`→native render; `log_command` deleted)
- [ ] The corresponding dead Tauri handlers (`get_entity`, `spatial_*`, `show_context_menu`, `log_command`) are removed from `commands.rs` and from `generate_handler!`
- [ ] `mcp_call`/`mcp_subscribe` remain Tauri commands
- [ ] No behavior regression: entity reads, spatial navigation, context menus still work end-to-end in the UI

## Tests
- [ ] `apps/kanban-app/ui/src/__tests__/no-direct-invoke.test.ts` — greps the source for `invoke("` and asserts only `mcp_call`/`mcp_subscribe` (and, until its task lands, `dispatch_command`) appear
- [ ] Per-action E2E test (Playwright): entity inspector loads (was `get_entity`); keyboard spatial navigation still moves focus (was `spatial_*`); right-click context menu appears (was `show_context_menu`)
- [ ] `cargo check -p kanban-app` passes after the handler deletions
- [ ] `npm test --prefix apps/kanban-app/ui` passes

## Workflow
- Use `/tdd` — write the grep test first to lock the contract; then migrate call sites until it passes.

Depends on: the `entity` server (read), the `focus` server (spatial-nav), the `window`/`app` server (context-menu render), and the `useDispatchCommand` rewrite (for `dispatch_command`).