---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffd80
title: Generate dynamic view and board switch commands from runtime data
---
## What

The palette and menu have no commands for switching views or switching between open boards. These were frontend-only commands that got lost when the command system moved to backend-driven resolution.

### Specific symptoms
1. No 'Switch to Tag Grid' or 'Switch to Task Grid' in the palette — view switching is missing entirely
2. No 'Switch to Board X' commands for open boards — board switching is missing
3. Open Board (File > Open Board) picks the folder but doesn't switch the current window to show that board

### Root cause
- View switch commands were generated dynamically in the frontend `globalCommands` from the views context
- Board switch commands were generated dynamically from the open boards list
- `commands_for_scope` only returns entity schema commands and global registry commands — it has no concept of dynamic runtime commands from views or boards data

### Implementation
- Add a `views` parameter to `commands_for_scope` (list of view definitions)
- Add a `boards` parameter (list of open boards from UIState)
- For each view: generate a command `view.switch:{view_id}` with name from view def, dispatches `ui.view.set` with `view_id` arg
- For each open board: generate a command `board.switch:{path}` with board name, dispatches `file.switchBoard` with path arg
- These appear in the palette as searchable commands

### Files to modify
- `swissarmyhammer-kanban/src/scope_commands.rs` — add views + boards parameters, generate dynamic commands
- `kanban-app/src/commands.rs` — pass views and boards to `commands_for_scope` in `list_commands_for_scope`
- `kanban-app/src/commands.rs` — fix Open Board result handling: after `OpenBoardDialog` triggers the dialog, the board switch via `open_and_notify` may deadlock (see related card)

## Acceptance Criteria
- [ ] Palette shows 'Board View', 'Task Grid', 'Tag Grid' (or whatever views are loaded)
- [ ] Palette shows open boards by name for switching
- [ ] Selecting a view in palette switches to it
- [ ] Selecting a board in palette switches to it
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Tests
- [ ] `scope_commands::tests` — add test: view commands appear when views are provided
- [ ] `scope_commands::tests` — add test: board switch commands appear when boards are provided
- [ ] `scope_commands::tests` — add test: view/board commands have correct names and IDs"
</invoke>