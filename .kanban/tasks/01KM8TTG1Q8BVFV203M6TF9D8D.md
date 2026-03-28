---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffe280
title: 'Remove AppState legacy: remove get_ui_context, clean up dead code'
---
Final cleanup: remove get_ui_context Tauri command (replaced by get_ui_state), update frontend callers in App.tsx, remove from invoke_handler in main.rs, and clean up dead code in state.rs and commands.rs.