---
assignees:
- claude-code
position_column: todo
position_ordinal: c280
title: 'command-scope.tsx: ActiveBoardPathProvider uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/lib/command-scope.tsx:22`\n\n`ActiveBoardPathProvider` has 2 props (`value`, `children`) defined inline. Extract to `interface ActiveBoardPathProviderProps`. #props-slop