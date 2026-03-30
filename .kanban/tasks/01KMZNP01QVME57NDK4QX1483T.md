---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: Generate dynamic view and board switch commands from runtime data
---
## What

`commands_for_scope` should generate dynamic commands from runtime data:

1. **View switching**: For each view loaded from the views context, generate a command like `view.switch:board-view` with name 'Board View'. These appear in the palette so users can switch views by name.

2. **Board switching**: For each open board in UIState, generate a command like `board.switch:/path/to/.kanban` with the board name. These appear in the palette so users can switch boards.

### Implementation
- Add a `views` parameter to `commands_for_scope` (or pass through KanbanContext)
- Walk the views context to generate view switch commands
- Walk UIState open_boards to generate board switch commands
- Each gets a unique ID, resolved name, and dispatches to the existing backend commands (`ui.view.set` with args, `file.switchBoard` with path arg)

### Where the data comes from
- Views: `KanbanContext → views_context() → list all views`
- Open boards: `UIState → open_boards()` + `recent_boards()` for names

## Acceptance Criteria
- [ ] Palette shows available views by name
- [ ] Palette shows open boards by name
- [ ] Selecting a view/board dispatches the correct backend command
- [ ] Tests for dynamic command generation"
<parameter name="assignees">[]