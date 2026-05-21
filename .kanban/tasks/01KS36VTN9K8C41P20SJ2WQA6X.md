---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8e80'
project: command-backends
title: '`window` MCP server (window ops + OS file open/reveal)'
---
## What

Create a new in-process MCP server `window` exposing window-level operations and OS-level file actions. (Board-file lifecycle is a SEPARATE task on this same server — `01KS612DV4W0N1X1RPXWAKMT4B`.)

REALITY CHECK (verified against `apps/kanban-app/src/commands.rs`): there is NO `apps/kanban-app/src/commands/window.rs` — all Tauri handlers live in the single `apps/kanban-app/src/commands.rs`, and the only window-ish handler is `create_window`. There are **no** `activate_window`/`set_window_position`/`get_window_position`/`get_monitors`/`close_window` handlers, and no `open_path`/`reveal_path` handlers. So `OpenNewWindow` ports `create_window`, but the rest of the window ops and both OS-file ops are **net-new** behavior to build (using `tauri`'s window/opener APIs), not a transport relocation. Scope accordingly.

Files:
- `crates/swissarmyhammer-window-service/Cargo.toml` — new workspace member, depends on `tauri`, `rmcp`, `swissarmyhammer-operations`
- `crates/swissarmyhammer-window-service/src/operations.rs` — `#[operation]` structs in two groups:
  - **window**: `OpenNewWindow` (`window.new`, ports `create_window` in `commands.rs`); `ActivateWindow`, `SetWindowPosition`, `GetWindowPosition`, `GetMonitors`, `CloseWindow` (NET-NEW — implement against the tauri window API; no existing handlers to port)
  - **OS file actions**: `OpenPath` (open a file in the OS default app — `attachment.open`), `RevealPath` (reveal in Finder/Explorer — `attachment.reveal`). Today these are direct OS calls inside the attachment command paths — relocate that logic here.
- `crates/swissarmyhammer-window-service/src/service.rs` — `WindowService` holding the tauri `AppHandle`
- bootstrap — `host.expose_rust_module("window", WindowService::new(app_handle.clone()))`
- `window_info.rs` (currently in `swissarmyhammer-commands`, deleted at cut-over): `WindowInfo` IS used (`crates/swissarmyhammer-kanban/src/dynamic_sources.rs:33`, `apps/kanban-app/src/menu.rs:6`), so relocating it is REQUIRED, not optional. Move it here (or to a small shared crate) and update those importers.

This consolidates window + OS-file shell concerns into the `window` server, per the "fewer servers" decision. `OpenNewWindow` and `OpenPath`/`RevealPath` are relocations; the other window ops are new construction.

## Acceptance Criteria
- [ ] `window` server registered at bootstrap with the window + OS-file op group
- [ ] `window.new` opens a new app window (porting `create_window`)
- [ ] `ActivateWindow`/`SetWindowPosition`/`GetWindowPosition`/`GetMonitors`/`CloseWindow` work (net-new, against the tauri window API)
- [ ] `attachment.open` (open in default app) and `attachment.reveal` (reveal in file manager) work through `OpenPath`/`RevealPath`
- [ ] `window_info.rs`/`WindowInfo` relocated out of `swissarmyhammer-commands`; `dynamic_sources.rs` and `menu.rs` updated to the new path
- [ ] `_meta` tree has all verbs in this group

## Tests
- [ ] `crates/swissarmyhammer-window-service/tests/integration/window_e2e.rs` — per-operation tests using a real tauri test app: `window.new` opens a window; window position get/set; monitor query; activate; close. `OpenPath`/`RevealPath` invoke the OS handler via an injectable spy/shim (define the seam so it's automated, not manual).
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-window-service` passes

## Workflow
- Use `/tdd` — write the per-operation integration tests first.

Prerequisite for: ui-commands plugin (`window.new`), kanban-misc-commands plugin (`attachment.open`/`reveal`), and the board-lifecycle task (`01KS612DV4W0N1X1RPXWAKMT4B`, same server). Depends on the operation-struct foundation. Can run in parallel with the other backend servers.