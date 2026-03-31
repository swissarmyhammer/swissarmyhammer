---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: app.command and app.palette are undocumented aliases
---
swissarmyhammer-kanban/src/commands/mod.rs:147-158\n\nBoth `app.command` and `app.palette` are registered to the same `CommandPaletteCmd` implementation. The alias relationship isn't documented — a future maintainer could think one is dead code.\n\nSuggestion: Add a brief comment explaining the alias, e.g. `// app.palette is an alias for app.command — both open the command palette`." #review-finding