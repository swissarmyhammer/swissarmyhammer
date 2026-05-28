---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbb80
project: command-backends
title: '`app` MCP server (app-shell actions: quit/about/help)'
---
## What

Create a new in-process MCP server `app` exposing genuine app-shell actions: quit, about, help.

REALITY CHECK (verified against `apps/kanban-app/src/commands.rs`): there is NO `apps/kanban-app/src/commands/application.rs` (handlers live in the single `apps/kanban-app/src/commands.rs`), and only `quit_app` exists today (`commands.rs:730`). There are **no** `ShowAbout`/`ShowHelp` Tauri handlers anywhere in `apps/kanban-app/src`. So `QuitApp` ports an existing handler, but `ShowAbout`/`ShowHelp` are **net-new** behavior to design and build (about dialog + help action), not a transport relocation. Scope accordingly.

NOTE: undo/redo are NOT here. They are store-layer concerns and live on the `store` MCP server (see the `store` server task), which exposes them over the shared `StoreContext`. The `app.undo`/`app.redo` *commands* route to `store.undo`/`store.redo`. Likewise `app.command/palette/search/dismiss` are UI-state toggles → `ui_state` server. This `app` server is only the true shell actions.

Files:
- `crates/swissarmyhammer-app-service/Cargo.toml` — new workspace member
- `crates/swissarmyhammer-app-service/src/lib.rs`
- `crates/swissarmyhammer-app-service/src/operations.rs` — operations: `QuitApp` (ports `commands.rs:730`), `ShowAbout` (NEW), `ShowHelp` (NEW)
- `crates/swissarmyhammer-app-service/src/service.rs`
- `apps/kanban-app/src/setup.rs` — `host.expose_rust_module("app", AppService::new(app_handle))`

`mcp_call` / `mcp_subscribe` Tauri commands stay Tauri (they are the MCP transport).

## Acceptance Criteria
- [ ] `app` registered as an in-process server at bootstrap
- [ ] `app.quit` routes through this server (replacing the existing `quit_app` Tauri handler at `commands.rs:730`)
- [ ] `app.about` and `app.help` work (net-new behavior — define the about dialog + help action)
- [ ] No undo/redo logic in this server (it lives in `store`)
- [ ] `_meta` tree complete; no behavior regression for quit

## Tests
- [ ] `crates/swissarmyhammer-app-service/tests/integration/app_e2e.rs` — per-operation tests using a real tauri test app: quit triggers the same shell behavior as `quit_app`; about/help trigger their (new) defined behavior
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-app-service` passes

## Workflow
- Use `/tdd`

Depends on the operation-struct foundation. (Cross-cutting undo/redo moved to the `store` server task.)