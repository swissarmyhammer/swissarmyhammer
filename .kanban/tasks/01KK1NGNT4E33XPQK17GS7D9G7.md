---
position_column: done
position_ordinal: ffffa080
title: 'W4: dispatch_command builds CommandContext without UIState on list_available_commands'
---
In `swissarmyhammer-kanban-app/src/commands.rs`, `list_available_commands` (line 556) builds a `CommandContext` for the dynamic availability check but does NOT set `ui_state` or the `KanbanContext` extension on it. This means commands whose `available()` method checks `ctx.ui_state` or `ctx.extension::<KanbanContext>()` will always return false during availability filtering, making them invisible in the palette.\n\nCurrently this is benign because UI commands check `ctx.target` / `ctx.scope_chain` (not ui_state) for availability, and kanban commands only use KanbanContext in `execute()` not `available()`. But it is a latent bug -- any future command that checks extensions in `available()` will be silently hidden.\n\nFile: swissarmyhammer-kanban-app/src/commands.rs:564-576 #review-finding #warning