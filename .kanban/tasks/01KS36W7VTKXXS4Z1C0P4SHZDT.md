---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 8f80
project: command-backends
title: '`app` MCP server (app-shell actions: quit/about/help)'
---
## What

Create a new in-process MCP server `app` exposing genuine app-shell actions: quit, about, help. Replaces the app-shell `#[tauri::command]` handlers in `apps/kanban-app/src/commands/application.rs`.

NOTE: undo/redo are NOT here. They are store-layer concerns and live on the `store` MCP server (see the `store` server task), which exposes them over the shared `StoreContext`. The `app.undo`/`app.redo` *commands* route to `store.undo`/`store.redo`. Likewise `app.command/palette/search/dismiss` are UI-state toggles → `ui_state` server. This `app` server is only the true shell actions.

Files:
- `crates/swissarmyhammer-app-service/Cargo.toml` — new workspace member
- `crates/swissarmyhammer-app-service/src/lib.rs`
- `crates/swissarmyhammer-app-service/src/operations.rs` — operations: `QuitApp`, `ShowAbout`, `ShowHelp`
- `crates/swissarmyhammer-app-service/src/service.rs`
- `apps/kanban-app/src/setup.rs` — `host.expose_rust_module("app", AppService::new(app_handle))`

`mcp_call` / `mcp_subscribe` Tauri commands stay Tauri (they are the MCP transport).

## Acceptance Criteria
- [ ] `app` registered as an in-process server at bootstrap
- [ ] `app.quit`, `app.about`, `app.help` route through this server (replacing the Tauri handlers)
- [ ] No undo/redo logic in this server (it lives in `store`)
- [ ] `_meta` tree complete; no behavior regression

## Tests
- [ ] `crates/swissarmyhammer-app-service/tests/integration/app_e2e.rs` — per-operation tests using a real tauri test app (quit/about/help trigger the expected shell behavior)
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-app-service` passes

## Workflow
- Use `/tdd`

Depends on the operation-struct foundation. (Cross-cutting undo/redo moved to the `store` server task.)