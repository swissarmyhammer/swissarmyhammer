---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffc80
title: Add tests for UIState open boards and window board management
---
ui_state.rs:430-505\n\nMultiple methods with no test coverage:\n- `add_open_board(path)` — adds to open list, deduplicates\n- `remove_open_board(path)` — removes and clears window assignments\n- `open_boards()` — returns the list\n- `set_window_board(label, path)` — per-window board assignment\n- `window_board(label)` — get board for window (None if empty)\n- `all_window_boards()` — all non-empty assignments\n\nTest cases:\n1. add_open_board adds and deduplicates\n2. remove_open_board removes from list\n3. remove_open_board clears window.board_path for affected windows\n4. set_window_board + window_board round-trip\n5. window_board returns None for unassigned window\n6. all_window_boards filters out empty board_path entries