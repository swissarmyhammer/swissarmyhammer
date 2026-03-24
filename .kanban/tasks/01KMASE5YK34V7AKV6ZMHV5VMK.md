---
assignees:
- claude-code
position_column: todo
position_ordinal: '898180'
title: 'entity-card.tsx: hardcoded parse-body-progress derive check'
---
**File:** `kanban-app/ui/src/components/entity-card.tsx:165`\n\n```ts\n(field.type as Record<string, unknown>).derive === \"parse-body-progress\"\n```\n\nCardFieldDispatch hardcodes that `parse-body-progress` computed fields render SubtaskProgress with the body field. Should be driven by `field.display` — e.g. a display type like `progress` that knows how to render itself. #field-special-case #review-finding