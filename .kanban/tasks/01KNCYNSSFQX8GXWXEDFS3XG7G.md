---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff180
title: Fix file.closeBoard — close current window, only remove board if last viewer
---
## What

`file.closeBoard` currently removes the board from `open_boards` unconditionally and resets window titles. The correct behavior is:

1. **Close the window** that dispatched the command (via `window.close()`)
2. **Only remove the board from `open_boards`** if no other window is still showing it
3. If it's the **last visible window**, keep it open but clear its board (macOS convention)

### Current flow (broken)

1. `CloseBoardCmd::execute` (`file_commands.rs:127-160`) calls `ui.remove_open_board(&path)` unconditionally — this removes the board from `open_boards` AND clears `board_path` for ALL windows showing it
2. Tauri handler (`commands.rs:1083-1104`) finds affected windows via `all_window_boards()`, calls `state.close_board()` to drop the BoardHandle, then resets window titles — but never closes any window

### Required changes

**`CloseBoardCmd::execute` (`file_commands.rs:127-160`):**
- Return the `window_label` in the `BoardClose` result so the Tauri handler knows WHICH window to close
- Do NOT call `ui.remove_open_board()` here — let the Tauri handler decide based on window count

**Tauri handler (`commands.rs:1083-1104`):**
1. Extract `window_label` from the `BoardClose` result (the requesting window)
2. Count how many windows show this board via `all_window_boards()`
3. If only one window shows this board → call `state.close_board()` to drop the BoardHandle AND `ui.remove_open_board()` to remove from open list
4. If multiple windows show it → just clear this window's `board_path` (don't remove from open list or drop handle)
5. Close the requesting window via `app.get_webview_window(&label).unwrap().close()` — UNLESS it's the last visible window, in which case keep it open with cleared board

**`remove_open_board` side effect note:** `remove_open_board` (`ui_state.rs:502-514`) already clears `board_path` for ALL windows showing that board. When only removing for the last viewer, this is correct. When NOT removing (multi-window case), we need to clear only the requesting window's `board_path` via `set_window_board(label, \"\")` instead.

### Files to modify
- `swissarmyhammer-kanban/src/commands/file_commands.rs` — `CloseBoardCmd::execute`: include `window_label` in result, remove `ui.remove_open_board()` call
- `kanban-app/src/commands.rs` — `BoardClose` handler (lines 1083-1104): implement conditional close/remove logic

## Acceptance Criteria
- [ ] `file.closeBoard` closes the current window
- [ ] Board stays in `open_boards` if another window is still showing it
- [ ] Board is removed from `open_boards` only when the last window showing it closes
- [ ] BoardHandle is dropped only when the last window showing it closes
- [ ] Last visible window is not closed (stays open with no board, macOS convention)

## Tests
- [ ] Update `file_commands.rs` test: `CloseBoardCmd` result includes `window_label`, does NOT call `remove_open_board`
- [ ] `cargo test -p swissarmyhammer-kanban -- file_commands` — passes
- [ ] `cargo nextest run -p kanban-app` — no regressions