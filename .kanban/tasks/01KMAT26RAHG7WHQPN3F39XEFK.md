---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
title: 'schema-context.tsx: hardcoded PRELOAD_TYPES list'
---
**File:** `kanban-app/ui/src/lib/schema-context.tsx:33`\n\n```ts\nconst PRELOAD_TYPES = [\"task\", \"column\", \"tag\", \"board\", \"swimlane\", \"actor\"];\n```\n\nEntity types to preload are hardcoded. Should be discovered from the backend or declared in board config. #field-special-case