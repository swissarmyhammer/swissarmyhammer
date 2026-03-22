---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: 'badge-list-display.tsx: hardcoded parse-body-tags detection'
---
**File:** `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx:26-28`\n\n```ts\nconst isComputedTags = field.type.kind === \"computed\" && (field.type as Record<string, unknown>).derive === \"parse-body-tags\";\n```\n\nSame pattern as multi-select-editor — hardcodes detection of computed tag fields to determine how to resolve and display values. Uses type cast to access `derive`. Should use field-level config to know the target entity type. #field-special-case