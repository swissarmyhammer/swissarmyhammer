# Plan 3 — Command Backends (domain servers)

**Kanban project:** `command-backends` · **Tier 1** · **Depends on:**
`store-service` (the `views` server writes through the shared `StoreContext`;
`window`/`ui_state`/`app` are not store-backed); the merged `operation_tool!`
macro.

The domain MCP servers a command's `execute`/`available` callbacks call —
**excluding** the generic entity layer (that's the `entity-service` plan) and
undo (the `store-service` plan). Each is one in-process rmcp server.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5EA17K4KDANFFRGW92QARF` | `views` MCP server (perspective + view state) | store-service substrate | 17 perspective ops + `set view`. State spans TWO crates: `PerspectiveContext` (`swissarmyhammer-perspectives`) + `ViewsContext` (`swissarmyhammer-views`) — pick one host crate, depend on the other, no struct moves/duplication; mutations on the unified changelog so `store.undo` reverts them. |
| `01KS5E9M7ZNPNA0E7GR1C9N42R` | `ui-state` MCP server (relocate UIState) | — | Relocates `UIState` out of (to-be-deleted) `swissarmyhammer-commands`; exposes inspector/palette/mode/rename/drag/keymap + app UI toggles; persists to the same on-disk store. |
| `01KS36VTN9K8C41P20SJ2WQA6X` | `window` MCP server (window ops + OS file open/reveal) | — | `window.new` (ports `create_window`) + activate/position/monitors/close (NET-NEW vs the tauri API — no existing handlers) + `OpenPath`/`RevealPath`; relocates `WindowInfo` (it IS used). There is NO `commands/window.rs` — handlers live in one `commands.rs`. |
| `01KS612DV4W0N1X1RPXWAKMT4B` | `window` server: board-file lifecycle (switch/close/new/open) | window ops task | `SwitchBoard`/`CloseBoard`/`NewBoard`/`OpenBoard` on the SAME `window` server; NewBoard/OpenBoard port the existing `*_board_dialog` handlers (incl. OS dialog); Switch/Close wrap existing `AppState` methods. |
| `01KS36W7VTKXXS4Z1C0P4SHZDT` | `app` MCP server (app-shell: quit/about/help) | — | `QuitApp` ports the existing `quit_app` handler; `ShowAbout`/`ShowHelp` are NET-NEW (no handlers today, no `application.rs`). NO undo/redo (those are on `store`). |

## Not here (moved/owned elsewhere)

- **Generic entity CRUD + clipboard + archive + search** → the `entity` server
  (plan 7, `entity-service`). The kanban tool keeps only domain ops; it needs
  **no new ops** for this effort (just shared-`StoreContext` wiring + feeding the
  notification bus).
- **undo/redo** → the `store` server (plan 1).
- **`focus`/spatial** (`ui.setFocus`) → the `spatial-nav` project
  (`01KS5MYQRB1E5HQ9JJ6TC7Z59S`).

## Key decisions baked in

- **Consolidated, not one-per-context**: board files + OS ops fold into `window`;
  app-shell is just quit/about/help; undo on `store`; generic entity on `entity`.
- `views` writes through the **shared** `StoreContext` so its changes land on the
  one undo stack. `ui_state`/`window`/`app` are not store-backed (no undo dep).
- `ui-state` **relocation** must precede the cut-over (plan 6) that deletes
  `swissarmyhammer-commands`.

## Cross-check

`kanban list tasks --filter '$command-backends'` → expect exactly these 5 tasks.
