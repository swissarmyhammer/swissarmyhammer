---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe380
title: No view switch commands in palette (Tag Grid, Task Grid missing)
---
## What

The palette has no commands to switch views. 'Tag Grid', 'Task Grid', 'Board View' are all missing. These were frontend globalCommands generated from the views context — lost when palette moved to backend-driven `list_commands_for_scope`.

### Fix
`commands_for_scope` needs to generate view switch commands from the views context. For each loaded view, add a command like 'Tag Grid' that dispatches `ui.view.set` with the view ID.

### Files to modify
- `swissarmyhammer-kanban/src/scope_commands.rs` — accept views list, generate view switch commands

## Acceptance Criteria
- [ ] Palette shows view names (Board View, Task Grid, Tag Grid)
- [ ] Selecting one switches the active view

## Tests
- [ ] scope_commands test: view commands generated from views list"
<parameter name="assignees">[]