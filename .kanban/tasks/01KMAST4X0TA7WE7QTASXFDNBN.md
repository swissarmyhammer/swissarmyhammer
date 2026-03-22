---
assignees:
- claude-code
position_column: todo
position_ordinal: '9680'
title: 'mention-pill.tsx: hardcoded entity type check for tag untag command'
---
**File:** `kanban-app/ui/src/components/mention-pill.tsx:86`\n\n```ts\nentityType === \"tag\" && taskId\n```\n\nAdds a `task.untag` context menu command only when entity type is \"tag\" and it's on a task. Entity commands should be schema-driven, not hardcoded per entity type. #field-special-case