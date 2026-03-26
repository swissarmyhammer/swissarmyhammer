---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffb180
title: 'board-view.tsx: hardcoded position field names'
---
**File:** `kanban-app/ui/src/components/board-view.tsx:84,92-93,204,222,308`\n\nHardcodes `\"position_column\"`, `\"position_ordinal\"`, `\"position_swimlane\"`, `\"order\"`, `\"name\"` in layout/sort logic. Position field names should come from schema configuration. #field-special-case