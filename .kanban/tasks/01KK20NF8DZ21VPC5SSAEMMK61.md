---
position_column: done
position_ordinal: ffff8180
title: ui.inspect falls back to first scope_chain entry without checking if it's inspectable
---
swissarmyhammer-kanban/src/commands/ui_commands.rs:26-28\n\nInspectCmd::execute() falls back to `ctx.scope_chain.first()` when no target is set. The scope chain can contain any moniker type (column, board, etc.), and the first entry is the innermost scope. This means the inspect command could receive a moniker like 'board:board' and push it onto the inspector stack even though it may not be meaningful to inspect.\n\nThe available() check (line 14-15) allows the command when the scope chain is non-empty, regardless of what types are in it.\n\nSuggestion: Consider filtering scope_chain entries to inspectable types, or at minimum document this is intentional. #review-finding #warning