---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffb080
title: 'avatar.tsx: hardcoded actor field names'
---
**File:** `kanban-app/ui/src/components/avatar.tsx:42-46`\n\nDirectly accesses `\"name\"`, `\"color\"`, `\"avatar\"` by hardcoded string. Should resolve display fields from the actor entity schema. #field-special-case