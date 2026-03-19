---
assignees:
- claude-code
position_column: done
position_ordinal: fffffff680
title: main window on_window_event creates WindowState with empty board_path
---
**main.rs:218-227**

When the main window fires Moved/Resized for the first time and no `windows["main"]` entry exists, the handler creates one with `board_path: std::path::PathBuf::new()` — an empty path. This is technically wrong: the main window always has a board loaded, but the geometry handler doesn't have access to which board it is. Later when `get_ui_context("main")` is called, it will return `board_path: ""`.

**Suggestion:** Either read the active board path from `state.active_board` inside the async block, or ensure `windows["main"]` is always initialized when a board is opened (in `auto_open_board` or `open_board`).

- [ ] Initialize windows["main"] entry when the first board is opened
- [ ] Or read active_board inside the on_window_event async block
- [ ] Verify board_path is never empty string in get_ui_context response