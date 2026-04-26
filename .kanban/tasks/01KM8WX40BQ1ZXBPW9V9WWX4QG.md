---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffb780
title: Route new_board_dialog and open_board_dialog through command system
---
Route new_board_dialog and open_board_dialog through dispatch_command_internal.\n\nThe dialog part is OS-level (fine as a Tauri command), but the board creation/opening should go through dispatch_command_internal().\n\n1. Make dispatch_command_internal pub(crate) in commands.rs\n2. Update open_and_notify in menu.rs to use dispatch_command_internal with file.switchBoard\n3. Keep board initialization (InitBoard) in handle_new_board as direct processing\n4. Verify board-opened event emission still works\n5. Run cargo nextest run -p kanban-app