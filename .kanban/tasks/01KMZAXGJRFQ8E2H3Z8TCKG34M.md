---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffff880
title: Move file/window/view commands to backend, emit events for UI effects
---
## What

Move frontend-only commands (file.newBoard, file.openBoard, window.new) to proper backend implementations. View switching and board switching commands should be generated from runtime data (views context, open boards). The backend emits events to tell the frontend which window/board/view to show.

### Current state (frontend slop)
- `file.newBoard`, `file.openBoard`, `window.new` are defined in app-shell.tsx with client-side `execute` handlers that call Tauri invocations directly
- View switching commands are hardcoded in frontend
- Board list commands are hardcoded in frontend

### Required
- Backend implementations for `file.newBoard`, `file.openBoard`, `window.new`
- These commands emit Tauri events (e.g. `new-board-dialog`, `open-board-dialog`, `create-window`) that the frontend listens for and acts on
- `commands_for_scope` returns dynamic view commands from the views context
- `commands_for_scope` returns dynamic board switch commands from UIState open boards
- Remove frontend globalCommands definitions — everything comes from backend

### Pattern
Backend command `execute()` → emit event → frontend listener acts on it. No `execute` callbacks in frontend command definitions.

## Acceptance Criteria
- [ ] All file/window commands dispatch through backend
- [ ] Backend emits events for UI effects (dialogs, window creation)
- [ ] View switching commands generated from views data
- [ ] Board switching commands generated from open boards data
- [ ] Frontend globalCommands array eliminated
- [ ] Tests for dynamic command generation"
<parameter name="assignees">[]