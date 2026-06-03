---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
project: command-cutover
title: Wire board-management callbacks in kanban-app; remove Tauri handlers
---
Wire list_open_boards/get_board_data callbacks in build_apphandle_shells to existing projection logic; make commands.rs projection helpers pub(crate); remove list_open_boards + get_board_data Tauri handlers + generate_handler! entries; update SpyAppShell.