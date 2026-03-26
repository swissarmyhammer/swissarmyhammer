---
position_column: done
position_ordinal: ffff9b80
title: 'W6: No integration test for dispatch_command Tauri command end-to-end'
---
The command dispatch pipeline (dispatch_command in commands.rs) is the critical path for all user actions but has no test. The unit tests in swissarmyhammer-kanban/src/commands/mod.rs test availability and execution of individual Command trait objects, which is good. But the Tauri `dispatch_command` function that wires together registry lookup + scope resolution + extension injection + availability check + execution + undoable wrapping is entirely untested.\n\nKey untested paths:\n- Scope chain fallback from explicit to stored focus\n- KanbanContext injection via set_extension\n- The undoable wrapper logic\n- Error formatting for missing commands\n\nFile: swissarmyhammer-kanban-app/src/commands.rs:477-545 #review-finding #warning