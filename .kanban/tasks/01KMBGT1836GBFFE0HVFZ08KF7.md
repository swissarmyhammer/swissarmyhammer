---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffde80
title: 'grid-view.tsx: GridFocusManager uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/grid-view.tsx:400`\n\n`GridFocusManager` has 3 props (`entityCommands`, `entityMoniker`, `fieldMoniker`) defined inline. Extract to `interface GridFocusManagerProps`. #props-slop