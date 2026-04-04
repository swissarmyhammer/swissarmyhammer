---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8380
title: Close window when its board is closed via file.closeBoard
---
## What

When a user closes a board (via `file.closeBoard` command or menu), any window displaying that board should be closed — not left as an empty shell with a reset title.

The fix is in `kanban-app/src/commands.rs` in the `BoardClose` handler (~line 1083–1104). Currently it:
1. Finds windows showing the closed board (`close_labels`)
2. Calls `state.close_board()` to remove the board handle
3. Resets window titles via `update_window_title(app, label, None)`

It should instead **close those windows** using `app.get_webview_window(&label)` → `window.close()` (Tauri 2 API). Exception: if the window being closed is the **last remaining window**, either leave it open (showing an empty/welcome state) or quit the app — match macOS conventions.

### Files to modify

- `kanban-app/src/commands.rs` — `BoardClose` handling block (~line 1083–1104): replace `update_window_title` calls with `window.close()` for secondary windows
- `kanban-app/src/main.rs` — verify `WindowEvent::CloseRequested` handler (~line 286) still cleans up UIState correctly when triggered programmatically

### Key references

- `create_window_impl()` at `kanban-app/src/commands.rs:655` — how windows are created
- `close_board()` at `kanban-app/src/state.rs:677` — board handle removal
- `WindowEvent::CloseRequested` at `kanban-app/src/main.rs:286` — existing close cleanup
- `all_window_boards()` at `swissarmyhammer-commands/src/ui_state.rs:558` — window→board mapping

## Acceptance Criteria

- [ ] Closing a board closes the window that was displaying it
- [ ] If the closed window is the last window, the app either quits or shows a welcome/empty state (not an empty board shell)
- [ ] Closing a board from a multi-window setup only closes the affected window(s), not all windows
- [ ] The `WindowEvent::CloseRequested` cleanup still fires correctly for programmatic closes

## Tests

- [ ] Add integration test in `kanban-app/src/commands.rs` (or `tests/`) that dispatches `file.closeBoard` and asserts the window is destroyed
- [ ] Test: closing a board when 2 windows are open only closes the one showing that board
- [ ] Run existing tests: `cargo nextest run -p swissarmyhammer-kanban -p kanban-app` — no regressions