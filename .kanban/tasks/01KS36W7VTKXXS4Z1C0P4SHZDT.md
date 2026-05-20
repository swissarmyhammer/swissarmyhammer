---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
position_column: todo
position_ordinal: 8f80
project: command-service
title: '`app` MCP server (replaces app-level Tauri commands)'
---
## What

Create a new in-process MCP server `app` exposing app-shell operations (quit, about, help, undo, redo, dismiss). Replaces the app-related `#[tauri::command]` handlers in `apps/kanban-app/src/commands/application.rs`.

Files:
- `crates/swissarmyhammer-app-service/Cargo.toml` — new workspace member
- `crates/swissarmyhammer-app-service/src/lib.rs`
- `crates/swissarmyhammer-app-service/src/operations.rs` — operations matching today's Tauri commands: `QuitApp`, `ShowAbout`, `ShowHelp`, `DismissModal`, `Undo`, `Redo`, etc.
- `crates/swissarmyhammer-app-service/src/service.rs` — service holding any needed state references (undo manager, modal coordinator)
- `apps/kanban-app/src/setup.rs` — `host.expose_rust_module("app", AppService::new(...))`

NOTE: `mcp_call` and `mcp_subscribe` Tauri commands in `apps/kanban-app/src/commands/mcp.rs` are NOT migrated — they ARE the MCP transport between the frontend and Rust. They stay as Tauri commands (per the user's design answer: "internal helpers as Tauri" is the practical exception for the transport itself).

## Acceptance Criteria
- [ ] Every non-transport Tauri command in `apps/kanban-app/src/commands/application.rs` has a corresponding `#[operation]` in `swissarmyhammer-app-service`
- [ ] Bootstrap registers `app` as an in-process server
- [ ] Undo/redo round-trips through the unified undo stack (see project `single-changelog`)
- [ ] No behavior regression vs. today's Tauri implementation

## Tests
- [ ] `crates/swissarmyhammer-app-service/tests/integration/app_e2e.rs` — per-operation tests using real tauri test app
- [ ] Undo/redo integration test: mutate state via a Command service execute; call `app.undo`; assert state reverted; call `app.redo`; assert state restored
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-app-service` passes

## Workflow
- Use `/tdd`