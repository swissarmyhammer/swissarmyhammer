---
assignees:
- claude-code
depends_on:
- 01KN5ENGH9DD3HGRHN49QG4P37
position_column: done
position_ordinal: ffffffffffffffffffffdc80
title: 'Phase 2: Rewrite UndoCmd/RedoCmd to use StoreContext'
---
Update undo_commands.rs to use require_extension::<StoreContext>() instead of EntityContext undo/redo.