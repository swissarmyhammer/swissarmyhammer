---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
position_column: todo
position_ordinal: '8e80'
project: command-service
title: '`window` MCP server (replaces window-related Tauri commands)'
---
## What

Create a new in-process MCP server `window` exposing window-level operations as a single operation tool. Replaces the existing `#[tauri::command]` window handlers (`activate_window`, `set_window_position`, `get_monitors`, plus any others in `apps/kanban-app/src/commands/window.rs`).

Files:
- `crates/swissarmyhammer-window-service/Cargo.toml` — new workspace member, depends on `tauri`, `rmcp`, `swissarmyhammer-operations`
- `crates/swissarmyhammer-window-service/src/lib.rs`
- `crates/swissarmyhammer-window-service/src/operations.rs` — one `#[operation]` struct per window action: `ActivateWindow`, `SetWindowPosition`, `GetWindowPosition`, `GetMonitors`, `OpenNewWindow`, `CloseWindow`, etc. — enumerate by reading the existing `apps/kanban-app/src/commands/window.rs`
- `crates/swissarmyhammer-window-service/src/service.rs` — `WindowService` holds an `AppHandle` (or whatever tauri injects today) and dispatches each verb to the matching tauri API
- `apps/kanban-app/src/setup.rs` (or wherever the bootstrap runs) — `host.expose_rust_module("window", WindowService::new(app_handle.clone()))`

This is a 1:1 port of the existing Tauri commands into rmcp operations on a single operation tool. No behavior change, just transport change.

## Acceptance Criteria
- [ ] Every Tauri command in `apps/kanban-app/src/commands/window.rs` has a corresponding `#[operation]` in `swissarmyhammer-window-service`
- [ ] Bootstrap registers `window` as an in-process server
- [ ] `tools/call("window", { op: "activate window", id: "main" })` works through the dispatcher
- [ ] `_meta` tree has `window` noun with all verbs
- [ ] No behavior regressions: every window action observable from today's UI works identically through the new server

## Tests
- [ ] `crates/swissarmyhammer-window-service/tests/integration/window_e2e.rs` — load a real tauri test app; execute each operation; assert observable state (window position, monitor list, etc.)
- [ ] `crates/swissarmyhammer-window-service/tests/operations_meta.rs` — snapshot the `_meta` tree
- [ ] `cargo test -p swissarmyhammer-window-service` passes

## Workflow
- Use `/tdd` — write the per-operation integration tests first; they will fail until each operation is implemented.

Independent of the Command service tasks except in spirit (this is part of the same "frontend talks only MCP" theme). Can run in parallel with the builtin plugin ports.