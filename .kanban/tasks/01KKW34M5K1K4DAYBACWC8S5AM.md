---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffaa80
title: dispatch_command always uses global active board, ignores per-window board in multi-window
---
kanban-app/src/commands.rs:852-854\n\n`dispatch_command` resolves the KanbanContext by calling `state.active_handle().await` — the global active board — not the per-window `board_path`. When two windows display different boards and a user executes a mutation command (task.create, task.move, etc.) in window B (which is showing board B but the global active is board A), the command executes against board A. This is a correctness bug: mutations silently target the wrong board.\n\nThe `handleSwitchBoard` callback in App.tsx calls `set_active_board` to sync the backend global, but this creates a race: if two windows switch boards nearly simultaneously, the global active will reflect whichever window called `set_active_board` last.\n\nSuggestion: `dispatch_command` should accept an optional `board_path` parameter and plumb it into the KanbanContext extension, the same way query commands now do via `resolve_handle`." #review-finding