---
assignees:
- claude-code
position_column: todo
position_ordinal: bb80
title: 'App.tsx: ViewRouter uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/App.tsx:658`\n\n`ViewRouter` has 2 props (`board`, `tasks`) defined inline. Extract to `interface ViewRouterProps`. #props-slop