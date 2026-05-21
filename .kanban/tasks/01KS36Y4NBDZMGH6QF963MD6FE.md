---
assignees:
- claude-code
depends_on:
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS36W7VTKXXS4Z1C0P4SHZDT
position_column: todo
position_ordinal: '9280'
project: command-cutover
title: 'Frontend: replace direct `invoke()` calls with `window` / `app` MCP dispatcher'
---
## What

Replace every frontend `invoke("activate_window", ...)`, `invoke("set_window_position", ...)`, `invoke("quit_app", ...)`, etc. with calls through the generic MCP dispatcher to the `window` / `app` servers. The Tauri command handlers in `apps/kanban-app/src/commands/window.rs` and `apps/kanban-app/src/commands/application.rs` are deleted.

Files (in `apps/kanban-app/ui/`):
- Grep all `invoke(` call sites in the frontend (`apps/kanban-app/ui/src/**/*.{ts,tsx}`) and migrate every non-transport call:
  - `invoke("activate_window", ...)` → `dispatcher.window.window.activate({ ... })`
  - `invoke("set_window_position", ...)` → `dispatcher.window.window.setPosition({ ... })`
  - `invoke("get_monitors")` → `dispatcher.window.window.getMonitors({})`
  - `invoke("quit_app")` → `dispatcher.app.app.quit({})`
  - etc. — enumerate by reading the call sites
- Leave `invoke("mcp_call", ...)` and `invoke("mcp_subscribe", ...)` as Tauri calls — they are the MCP transport itself.

Files (in `apps/kanban-app/src/`):
- `apps/kanban-app/src/commands/window.rs` — delete; its functionality lives in `swissarmyhammer-window-service`
- `apps/kanban-app/src/commands/application.rs` — delete (except non-MCP-related plumbing)
- `apps/kanban-app/src/commands/mcp.rs` — keep (transport)
- `apps/kanban-app/src/lib.rs` (or `main.rs`) — remove the deleted commands from `tauri::Builder::invoke_handler(generate_handler![...])`

## Acceptance Criteria
- [ ] Every frontend `invoke()` call site that isn't `mcp_call`/`mcp_subscribe` is migrated
- [ ] `apps/kanban-app/src/commands/window.rs` is deleted (or reduced to non-invoke-handler helpers if any)
- [ ] `apps/kanban-app/src/commands/application.rs` is deleted (or reduced)
- [ ] Tauri `generate_handler!` macro no longer references the deleted commands
- [ ] No behavior regression: window operations and app operations still work end-to-end in the UI

## Tests
- [ ] `apps/kanban-app/ui/src/__tests__/no-direct-invoke.test.ts` — code-quality test that greps the source for `invoke("` and asserts only allowed names appear (`mcp_call`, `mcp_subscribe`)
- [ ] Per-action E2E test (Playwright): trigger UI actions that previously called Tauri commands directly; observe the same effect (window resize, monitor query result, quit dialog appears)
- [ ] `cargo check -p kanban-app` passes after the deletions
- [ ] `npm test --prefix apps/kanban-app/ui` passes

## Workflow
- Use `/tdd` — write the grep test first to lock the contract; then migrate call sites until it passes.

Depends on the `window` and `app` MCP servers existing.