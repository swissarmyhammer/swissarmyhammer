---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: Route new_board_dialog and open_board_dialog through command system
---
## What

`new_board_dialog` and `open_board_dialog` are Tauri commands that open native file dialogs and then create/open boards directly via `menu.rs` helpers, bypassing the command system. The dialog is OS-level (fine), but the board creation/opening after the dialog should go through `file.newBoard` / `file.openBoard` commands.

### Current path
1. Frontend calls `invoke("new_board_dialog")` or `invoke("open_board_dialog")`
2. Rust opens native folder picker
3. On selection, calls `menu::trigger_new_board()` / `menu::trigger_open_board()`
4. These call `AppState::open_board()` directly and emit `board-opened` / `board-changed` events

### Fix
After the dialog returns a path, dispatch `file.openBoard` (or `file.newBoard`) through the command system instead of calling `open_board()` directly. The native dialog stays as a Tauri command (it's OS-level), but the mutation goes through commands.

Alternatively, keep the dialog Tauri commands but have them call `dispatch_command` internally after the dialog completes.

## Acceptance Criteria
- [ ] Board creation after New Board dialog goes through command system
- [ ] Board opening after Open Board / Open Recent goes through command system
- [ ] Native dialogs still work

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] Manual: File > New Board, File > Open Board work correctly