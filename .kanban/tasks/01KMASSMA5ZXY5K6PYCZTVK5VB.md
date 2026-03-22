---
assignees:
- claude-code
position_column: todo
position_ordinal: '9080'
title: 'board-view.tsx: hardcoded position field names'
---
**File:** `kanban-app/ui/src/components/board-view.tsx:84,92-93,204,222,308`\n\nHardcodes `\"position_column\"`, `\"position_ordinal\"`, `\"position_swimlane\"`, `\"order\"`, `\"name\"` in layout/sort logic. Position field names should come from schema configuration. #field-special-case