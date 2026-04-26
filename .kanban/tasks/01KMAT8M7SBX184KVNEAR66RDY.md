---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffe280
title: 'mention-pill.tsx: hardcoded "color" and "description" field names'
---
**File:** `kanban-app/ui/src/components/mention-pill.tsx:63,65`\n\n```ts\nconst color = entity ? getStr(entity, \"color\", \"888888\") : \"888888\";\ngetStr(entity, \"description\") || undefined\n```\n\nAssumes all mentionable entities have `\"color\"` and `\"description\"` fields. Entity schemas should declare which fields provide color and description for mention rendering (e.g. a `mention_color_field` property). #field-special-case