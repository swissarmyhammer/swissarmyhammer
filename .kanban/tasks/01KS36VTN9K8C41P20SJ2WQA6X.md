---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8e80'
project: command-backends
title: '`window` MCP server (window ops + board-file lifecycle + OS file ops)'
---
## What

Create a new in-process MCP server `window` exposing window-level operations, board-file lifecycle, and OS-level file actions — i.e. the host-shell capabilities that `window.new`, `file.*`, and `attachment.open`/`attachment.reveal` commands depend on. Replaces the `#[tauri::command]` handlers in `apps/kanban-app/src/commands/window.rs` and the board/file Tauri events emitted from `apps/kanban-app/src/commands/file_commands.rs` paths.

Files:
- `crates/swissarmyhammer-window-service/Cargo.toml` — new workspace member, depends on `tauri`, `rmcp`, `swissarmyhammer-operations`
- `crates/swissarmyhammer-window-service/src/operations.rs` — `#[operation]` structs in three groups:
  - **window**: `ActivateWindow`, `SetWindowPosition`, `GetWindowPosition`, `GetMonitors`, `OpenNewWindow` (`window.new`), `CloseWindow` — enumerate by reading `apps/kanban-app/src/commands/window.rs`
  - **board lifecycle**: `SwitchBoard`, `CloseBoard`, `NewBoard`, `OpenBoard` (the `file.*` commands; `openBoard` shows an OS file-open dialog, `newBoard` creates a board file). Today these emit Tauri events from `file_commands.rs` — port the actual file/window I/O here.
  - **OS file actions**: `OpenPath` (open a file in the OS default app — `attachment.open`), `RevealPath` (reveal in Finder/Explorer — `attachment.reveal`). Today these are direct OS calls in the attachment commands.
- `crates/swissarmyhammer-window-service/src/service.rs` — `WindowService` holding the tauri `AppHandle`
- bootstrap — `host.expose_rust_module("window", WindowService::new(app_handle.clone()))`
- If `window_info.rs` (currently in `swissarmyhammer-commands`, deleted at cut-over) is needed, relocate it here.

This consolidates all OS/window/board-file shell concerns into one `window` server, per the "fewer servers" decision. No behavior change — transport change + relocation.

## Acceptance Criteria
- [ ] Every Tauri command in `apps/kanban-app/src/commands/window.rs` has a corresponding `#[operation]`
- [ ] Board lifecycle (`switchBoard`/`closeBoard`/`newBoard`/`openBoard`) works through this server, including the OS file-open dialog for `openBoard`
- [ ] `attachment.open` (open in default app) and `attachment.reveal` (reveal in file manager) work through `OpenPath`/`RevealPath`
- [ ] `window.new` opens a new app window
- [ ] `window_info.rs` (if used) relocated out of `swissarmyhammer-commands`
- [ ] `_meta` tree has all verbs; no behavior regression

## Tests
- [ ] `crates/swissarmyhammer-window-service/tests/integration/window_e2e.rs` — per-operation tests using a real tauri test app: window position/monitors/activate; `window.new` opens a window; `newBoard` creates a board file; `openBoard` dialog (mock the picker); `OpenPath`/`RevealPath` invoke the OS handler (assert via a spy/shim)
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-window-service` passes

## Workflow
- Use `/tdd` — write the per-operation integration tests first.

Prerequisite for: file-commands plugin, ui-commands plugin (`window.new`), kanban-misc-commands plugin (`attachment.open`/`reveal`). Depends on the operation-struct foundation. Can run in parallel with the other backend servers.