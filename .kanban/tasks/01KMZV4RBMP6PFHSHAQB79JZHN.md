---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe480
title: No board switch commands in palette
---
## What

No commands to switch between open boards in the palette. Was generated dynamically from open boards list in frontend — lost in backend migration.

### Fix
`commands_for_scope` needs to generate board switch commands from UIState open boards. For each open board, add a command with the board name that dispatches `file.switchBoard`.

### Files to modify
- `swissarmyhammer-kanban/src/scope_commands.rs` — read open boards from UIState, generate switch commands

## Acceptance Criteria
- [ ] Palette shows open board names
- [ ] Selecting one switches to that board

## Tests
- [ ] scope_commands test: board switch commands generated from open boards list"
<parameter name="assignees">[]