---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffee80
title: No tests for drag session commands or cross-board transfer
---
**kanban-app/src/commands.rs (new functions)**\n\nSix new backend functions were added (`start_drag_session`, `cancel_drag_session`, `complete_drag_session`, `invoke_dispatch`, `cross_board_transfer`, `flush_and_emit_for_handle`) with zero test coverage. The existing test count (389) is unchanged.\n\nKey scenarios that need tests:\n- Start/cancel session lifecycle\n- Same-board complete (delegates to task.move)\n- Cross-board transfer (task appears on target, removed from source)\n- Cross-board copy (task appears on target, remains on source)\n- Double-complete or cancel-after-complete (session already taken)\n- Ordinal computation in target column\n\n**Suggestion:** Add unit tests for `DragSession` state management and integration tests for cross-board transfer using the existing `setup()` test pattern from `task/mv.rs`.