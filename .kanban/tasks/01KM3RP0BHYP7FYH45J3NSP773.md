---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffd280
title: Board selection not persisting across restarts and hot reloads
---
## What
Opening a board works, but the board selection is lost on app restart or hot reload. The new per-window config system (`config.windows["main"].board_path`) is not being written when a board is first opened via the main window.

**Root cause hypothesis:**
The main window opens boards via `open_board` (Tauri command) which sets `state.active_board` in memory but does NOT write to `config.windows["main"].board_path`. The `switch_board` command does persist, but the initial board open on startup (via `auto_open_board`) and the `board-opened` event handler in `App.tsx` both go through `open_board`, not `switch_board`.

**Paths that should persist but don't:**
1. `auto_open_board` on startup — opens boards from `config.open_boards` but doesn't create `windows["main"]` entry
2. `open_board` Tauri command — sets active_board in memory, doesn't touch `config.windows`
3. `board-opened` event in App.tsx — sets `activeBoardPath` state but doesn't call `switch_board`

**Paths that DO persist:**
- `handleSwitchBoard` in App.tsx — calls `switch_board` which persists
- `create_window` — creates `windows[label]` entry

**Fix approach:**
- `open_board` (or `auto_open_board`) should ensure `windows["main"]` has the correct `board_path` when a board is opened on the main window
- OR the frontend mount logic should call `switch_board` after restoring from `get_ui_context`
- The simplest fix: in `App.tsx`, after `open_board` succeeds in the mount effect, call `switch_board` to persist the mapping

## Acceptance Criteria
- [ ] Open board A → quit → restart → board A is shown (not "no board loaded")
- [ ] Open board A → hot reload → board A is shown
- [ ] Open board A, switch to board B → restart → board B is shown
- [ ] Secondary windows restore their board on restart

## Tests
- [ ] Manual: open a board, quit app, relaunch — same board loads
- [ ] Manual: open a board, hot reload — same board loads
- [ ] `cargo nextest run -p kanban-app` passes