---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbe80
title: Add tests for UIState persistence and window management
---
swissarmyhammer-commands/src/ui_state.rs\n\nCoverage: 46.3% (146/315 lines)\n\nUncovered functions:\n- load / read_from_file / save (persistence round-trip, lines 212-263)\n- inspect / inspector_close / inspector_close_all / set_inspector_stack (inspector management)\n- add_open_board / remove_open_board / set_window_board / window_board / all_window_boards (multi-window board tracking)\n- touch_recent / recent_boards / set_most_recent_board / most_recent_board (recent boards list)\n- save_window_geometry / update_window_geometry / get_window_state / all_windows (window geometry persistence)\n- to_json (serialization, lines 893-923)\n\nWhat to test: Construct UIState, exercise each method, verify state mutations. Test save/load round-trip with temp file. Test inspector stack push/pop. Test window geometry save/restore. #coverage-gap