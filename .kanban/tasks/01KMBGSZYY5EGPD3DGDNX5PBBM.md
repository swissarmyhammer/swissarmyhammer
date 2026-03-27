---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff80
title: 'mention-pill.tsx: PillInner uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/mention-pill.tsx:140`\n\n`PillInner` has 7 props (`slug`, `prefix`, `color`, `tooltipText`, `richTooltip`, `scopeMoniker`, `className`) defined inline. Extract to `interface PillInnerProps`. #props-slop