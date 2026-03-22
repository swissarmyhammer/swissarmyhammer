---
assignees:
- claude-code
position_column: todo
position_ordinal: 8f80
title: 'avatar.tsx: hardcoded actor field names'
---
**File:** `kanban-app/ui/src/components/avatar.tsx:42-46`\n\nDirectly accesses `\"name\"`, `\"color\"`, `\"avatar\"` by hardcoded string. Should resolve display fields from the actor entity schema. #field-special-case