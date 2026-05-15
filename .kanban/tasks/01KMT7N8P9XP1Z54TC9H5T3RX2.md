---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffe80
title: Add tests for UIState window management (clear, remove, geometry, restore)
---
ui_state.rs:580-681\n\nWindow management methods with no test coverage:\n- `clear_windows()` — wipes all window state\n- `remove_window(label)` — removes one window\n- `restore_boards(open_boards)` — populates open_boards if empty\n- `save_window_geometry(label, x, y, w, h, maximized)` — stores geometry\n- `get_window_state(label)` — retrieves full WindowState\n- `all_windows()` — all window states\n\nTest cases:\n1. save_window_geometry + get_window_state round-trip\n2. remove_window removes the entry\n3. clear_windows removes all entries\n4. restore_boards populates when empty, no-ops when not empty\n5. all_windows returns all entries