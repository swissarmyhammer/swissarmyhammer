---
assignees:
- claude-code
position_column: todo
position_ordinal: '9580'
title: 'entity-card.tsx: hardcoded type.kind checks in CardFieldDispatch'
---
**File:** `kanban-app/ui/src/components/entity-card.tsx:134,147,165`\n\nCardFieldDispatch checks `field.type.kind === \"markdown\"`, `field.display === \"badge-list\"`, and `field.type.kind === \"computed\"` for special rendering. Should dispatch on `field.display` through a single dispatch path, same as CellDispatch. #field-special-case