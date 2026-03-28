---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffb580
title: 'left-nav.tsx: hardcoded view kind to icon mapping'
---
**File:** `kanban-app/ui/src/components/left-nav.tsx:10,23`\n\nSwitch statement maps view kinds (`\"kanban\"`, `\"grid\"`, `\"list\"`, `\"tag\"`, etc.) to Lucide icons. View definitions already have an `icon` property — should use the same dynamic icon lookup as field icons. #field-special-case