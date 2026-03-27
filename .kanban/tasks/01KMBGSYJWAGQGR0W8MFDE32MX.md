---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8280
title: 'command-palette.tsx: ResultRow uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/command-palette.tsx:483`\n\n`ResultRow` has 6 props defined inline. Extract to `interface ResultRowProps`. #props-slop