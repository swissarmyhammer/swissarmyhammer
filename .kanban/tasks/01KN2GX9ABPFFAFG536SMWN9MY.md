---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: 'Bug: Reset Windows should close secondary windows and reset geometry, not restart the app'
---
## What

"Reset Windows" (`app.resetWindows`) calls `app.restart()`, which crashes because it kills the process mid-dispatch. But the real problem is: why restart at all? Reset should just:

1. Close all secondary windows (everything except `main` and `quick-capture`)
2. Reset main window geometry to defaults (size + position)
3. Clear saved window state from UIState

No restart. The app stays running. The user's board, entity state, undo stack — all preserved.

### Current (wrong)
- `ResetWindowsCmd` (`swissarmyhammer-kanban/src/commands/app_commands.rs:92`) calls `ui.clear_windows()` and returns `{ "ResetWindows": true }`
- `dispatch_command_internal` (`kanban-app/src/commands.rs` ~line 1145) catches the result and calls `app.restart()` — hard crash

### Fix
- `dispatch_command_internal` — replace `app.restart()` with: iterate `app.webview_windows()`, close everything except `main` and `quick-capture`, reset main window size/position to defaults
- `reset_windows` Tauri command (`kanban-app/src/commands.rs:595`) — same: close secondary windows, reset main geometry, clear UIState. Remove `app.restart()`.
- `ResetWindowsCmd` (`app_commands.rs`) — `ui.clear_windows()` is fine, just the Tauri-layer handler needs to stop restarting

### Files to modify
- `kanban-app/src/commands.rs` — `dispatch_command_internal` ResetWindows handler (~line 1145) and `reset_windows` (~line 595): close secondary windows, reset main geometry instead of restarting
- `swissarmyhammer-kanban/src/commands/app_commands.rs` — no change needed (already clears UIState)

## Acceptance Criteria
- [ ] "Reset Windows" does not crash or restart the app
- [ ] All secondary windows close
- [ ] Main window resets to default size and centered position
- [ ] Saved window state is cleared (next restart won't restore old geometry)
- [ ] Board data, undo stack, entity state all preserved

## Tests
- [ ] `cargo nextest run -p kanban-app` — no regressions
- [ ] Manual: open 2 secondary windows, invoke "Reset Windows" → secondary windows close, main window resets to default size
- [ ] Manual: quit and relaunch after reset → main window at default position, no secondary windows restored