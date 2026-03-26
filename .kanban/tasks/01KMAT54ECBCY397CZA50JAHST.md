---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'grid-view.tsx: defaults entity type to "task"'
---
**File:** `kanban-app/ui/src/components/grid-view.tsx:43`\n\n```ts\nconst entityType = view.entity_type ?? \"task\";\n```\n\nDefaults to \"task\" when a view definition doesn't declare an entity type. Views should always declare their entity type — this fallback hides misconfiguration. #field-special-case