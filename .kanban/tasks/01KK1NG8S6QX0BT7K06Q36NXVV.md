---
position_column: done
position_ordinal: n0
title: 'W3: Repeated boilerplate for KanbanContext extraction in every command'
---
Every command implementation in swissarmyhammer-kanban/src/commands/ repeats the exact same 3-line pattern to extract KanbanContext:\n\n```rust\nlet kanban = ctx\n    .extension::<KanbanContext>()\n    .ok_or_else(|| CommandError::ExecutionFailed(\"KanbanContext not available\".into()))?;\n```\n\nThis appears 9 times across task_commands.rs, entity_commands.rs, column_commands.rs, and app_commands.rs. Similarly, every UI command repeats the UIState extraction pattern. A helper method on CommandContext (e.g. `ctx.require_extension::<T>()`) or a helper function in the commands module would eliminate this duplication and make the error message consistent.\n\nFile: swissarmyhammer-kanban/src/commands/*.rs #review-finding #warning