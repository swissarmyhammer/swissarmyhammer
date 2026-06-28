---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbc80
project: command-backends
title: '`ui-state` MCP server (relocate UIState out of swissarmyhammer-commands)'
---
## What

Build an in-process MCP server `ui-state` that wraps the existing `UIState` struct, exposing the UI-state mutations that `ui.*`, `settings.keymap.*`, `drag.*`, and the UI-toggle subset of `app.*` commands depend on. Today these commands reach `UIState` directly via `ctx.require_extension::<UIState>()` — there is NO MCP surface, so this server is net-new and is a hard prerequisite for the ui-commands and app-shell-commands plugins.

Critical relocation: `UIState` currently lives in `crates/swissarmyhammer-commands/src/ui_state.rs`, and that crate is DELETED in the cut-over. So this task must **move `UIState` to a surviving home** before the cut-over can proceed. Put it in the new server crate (or a small `swissarmyhammer-ui-state` crate if other code needs the type). Same applies to `window_info.rs` if `UIState` depends on it (window server task may claim that instead).

Files:
- `crates/swissarmyhammer-ui-state/Cargo.toml` (or fold into the server crate) — new home for the relocated `UIState`
- `crates/swissarmyhammer-ui-state/src/state.rs` — the relocated `UIState` struct (moved verbatim from `swissarmyhammer-commands/src/ui_state.rs`; update all importers)
- `crates/swissarmyhammer-ui-state/src/operations.rs` — `#[operation]` structs, one per UI mutation:
  - inspector: `Inspect`, `InspectorClose`, `InspectorCloseAll`, `InspectorSetWidth`
  - palette: `PaletteOpen`, `PaletteClose`
  - mode/keymap: `SetKeymapMode` (covers settings.keymap.vim/cua/emacs via a `mode` param)
  - rename: `StartRename`
  - drag: `DragStart`, `DragCancel`, `DragComplete`
  - app-ui toggles: `ShowCommand`, `ShowPalette`, `ShowSearch`, `Dismiss` (the app.command/palette/search/dismiss commands are UI-state toggles, not app-shell actions)
- `crates/swissarmyhammer-ui-state/src/service.rs` — `UiStateServer` holding the `UIState` (file-backed at `~/.swissarmyhammer/ui-state.json` as today)
- bootstrap — `host.expose_rust_module("ui_state", UiStateServer::new(...))`

NOTE — `ui.setFocus` / `SetFocus` is NOT on this server. Spatial focus is owned by the `focus` MCP server in the **spatial-nav** project (`01KS5MYQRB1E5HQ9JJ6TC7Z59S`); `ui.setFocus` routes there. `ui-state` owns no focus op (this resolves a prior double-ownership: the catalog, plan-03 "Not here", and plan-04 all route `ui.setFocus` to `focus`).

Behavior is a 1:1 port of today's `UIState` methods (`inspect`, `inspector_close`, `set_palette_open`, `set_keymap_mode`, `start_drag`/`take_drag`/`cancel_drag`, etc.) into rmcp operations. No behavior change.

## Acceptance Criteria
- [ ] `UIState` no longer lives in `swissarmyhammer-commands`; every importer updated to the new crate
- [ ] `ui_state` registered as an in-process server at bootstrap
- [ ] Every `UIState` mutating method has a corresponding `#[operation]` (EXCEPT focus — see note; if `UIState` currently has a focus setter, that capability moves to the `focus` server, not here)
- [ ] `tools/call("ui_state", { op: "inspect", ... })` round-trips and persists to the same on-disk location as today
- [ ] Drag, keymap, palette, inspector, rename state all reachable via MCP
- [ ] No `SetFocus`/focus op on `ui_state`
- [ ] `_meta` operations tree complete

## Tests
- [ ] `crates/swissarmyhammer-ui-state/tests/integration/ui_state_e2e.rs` — per-operation tests: inspect → assert inspector stack; set_keymap_mode → assert active keymap; drag start→complete → assert session transitions; palette open/close → assert flag. Real server, observe persisted state.
- [ ] `_meta` snapshot test (asserts no `SetFocus` op present)
- [ ] `cargo test -p swissarmyhammer-ui-state` passes

## Workflow
- Use `/tdd` — write per-operation tests first.

Prerequisite for: ui-commands plugin, app-shell-commands plugin. Depends on the operation-struct foundation + the `_meta`/operation-tool macro from plugin-arch.