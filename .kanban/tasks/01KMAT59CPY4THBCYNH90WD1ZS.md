---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffe180
title: 'views-context.tsx: hardcoded "view" entity type check'
---
**File:** `kanban-app/ui/src/lib/views-context.tsx:70,73,76`\n\n```ts\nif (event.payload.entity_type === \"view\") refresh();\n```\n\nHardcodes \"view\" as the entity type that triggers view list refresh. Three separate event handlers all check the same hardcoded string. #field-special-case