# Plan 3 — Command Backends (domain servers)

**Kanban project:** `command-backends` · **Tier 1** · **Depends on:**
`store-service` (views + kanban-ext write through the shared `StoreContext`);
the merged `operation_tool!` macro.

The domain MCP servers a command's `execute`/`available` callbacks actually
call. Investigation found only `kanban` exists today; the rest are net-new (or
an extension). Each is one in-process rmcp server.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5EA17K4KDANFFRGW92QARF` | `views` MCP server (perspective + view state) | store-service substrate | Exposes the 17 perspective ops + `set view` over the existing `PerspectiveStore`/`ViewStore`; mutations captured by the unified changelog so `store.undo` reverts them; no duplicate state. |
| `01KS5E9M7ZNPNA0E7GR1C9N42R` | `ui-state` MCP server (relocate UIState) | — | Relocates `UIState` out of (to-be-deleted) `swissarmyhammer-commands`; exposes inspector/palette/mode/focus/rename/drag/keymap + app UI toggles; persists to the same JSON. |
| `01KS36VTN9K8C41P20SJ2WQA6X` | `window` MCP server (window ops + board-file lifecycle + OS file ops) | — | window activate/position/monitors/new + board switch/close/new/open (incl. OS dialog) + OpenPath/RevealPath; replaces window/file Tauri handlers. |
| `01KS36W7VTKXXS4Z1C0P4SHZDT` | `app` MCP server (app-shell: quit/about/help) | — | `QuitApp`/`ShowAbout`/`ShowHelp` only; NO undo/redo (those are on `store`). |
| `01KS5EAD57PCBFJGMVB74FF4MK` | Extend `kanban` MCP tool: clipboard (cut/copy/paste) + archive/unarchive | store-service substrate | Adds `archive task`/`unarchive task` + `cut`/`copy`/`paste` (wraps `PasteMatrix`) to the existing kanban tool; undoable via the shared stack; preserves drag-vs-paste. |

## Key decisions baked in

- **Consolidated, not one-per-context**: clipboard + archive fold into `kanban`
  (it already owns generic entity CRUD); board files + OS ops fold into
  `window`; undo/redo are on `store` (plan 1), not `app`.
- `views` + `kanban-ext` write through the **shared** `StoreContext` so their
  changes land on the one undo stack. `ui_state`/`window`/`app` are not
  store-backed (no undo dep).
- `ui-state` **relocation** must precede the cut-over (plan 6) that deletes
  `swissarmyhammer-commands`.

## Cross-check

`kanban list tasks --filter '$command-backends'` → expect exactly these 5 tasks.
