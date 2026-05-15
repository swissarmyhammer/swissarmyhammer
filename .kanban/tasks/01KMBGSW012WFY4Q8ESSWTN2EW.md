---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffdc80
title: 'focus-scope.tsx: FocusScopeInner uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/focus-scope.tsx:87`\n\n`FocusScopeInner` has 6 props (`moniker`, `isDirectFocus`, `onClick`, `children`, `className`, `style`) defined inline. Extract to `interface FocusScopeInnerProps`. #props-slop