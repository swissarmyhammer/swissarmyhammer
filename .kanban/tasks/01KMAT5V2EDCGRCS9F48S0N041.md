---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffe680
title: 'quick-capture.tsx: hardcoded "board" entity type and "task.add" command'
---
**File:** `kanban-app/ui/src/components/quick-capture.tsx:92`\n\n```ts\nif (event.payload.entity_type === \"board\") loadBoards();\n```\n\nHardcodes \"board\" entity type check for refresh trigger. Also constructs board entities with hardcoded `\"name\"` field. #field-special-case