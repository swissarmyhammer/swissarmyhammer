---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
project: spatial-nav
title: '`focus`/`spatial` MCP server: expose SpatialRegistry/SpatialState for the command-driven UI'
---
## What

Give spatial navigation an MCP face so the command-driven, MCP-everywhere UI reaches it like any other service. Today the frontend calls `spatial_focus`, `spatial_navigate`, `spatial_push_layer`, `spatial_clear_focus`, `spatial_focus_lost` as **Tauri commands**, backed by `SpatialRegistry` + `SpatialState` (`swissarmyhammer-focus`, held in `AppState`). The Command Service work needs this for `ui.setFocus` (and the spatial-nav commands) to route through MCP instead of Tauri.

Files:
- `crates/swissarmyhammer-focus/src/server.rs` (or a thin mcp crate) — `FocusServer`/`SpatialServer` over `SpatialRegistry` + `SpatialState`
- `operations.rs` — `#[operation]` structs mirroring the current Tauri commands: `Focus`, `Navigate { direction }`, `PushLayer`, `ClearFocus`, `FocusLost`, `RegisterRect`/`SetFocus` as needed by the FocusScope model
- bootstrap — `host.expose_rust_module("focus", FocusServer::new(...))`

This is coordinated with the Command Service plans (`command-service` etc.):
- `ui.setFocus` (in the ui-commands plugin) routes to this `focus` server.
- The 5 `spatial_*` frontend Tauri calls migrate to the `focus` server during the frontend cut-over.

Owned by the `spatial-nav` project because the spatial model + its evolution live here; the Command Service plans only consume it.

## Acceptance Criteria
- [ ] `focus` registered as an in-process MCP server over `SpatialRegistry`/`SpatialState`
- [ ] Every `spatial_*` Tauri command has a corresponding MCP operation
- [ ] `ui.setFocus` can route through `focus` end-to-end
- [ ] `_meta` operations tree complete; no behavior regression vs the Tauri commands

## Tests
- [ ] `crates/swissarmyhammer-focus/tests/integration/focus_server_e2e.rs` — per-operation: focus an item, navigate by direction, push/pop a layer, clear focus; assert resulting focus state matches the Tauri-driven behavior
- [ ] `cargo test -p swissarmyhammer-focus` passes

## Workflow
- Use `/tdd`

Cross-project: prerequisite for the command-service `ui.setFocus` routing and the frontend cut-over's removal of the `spatial_*` Tauri commands.