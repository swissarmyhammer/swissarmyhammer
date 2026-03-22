---
assignees:
- claude-code
position_column: todo
position_ordinal: '9180'
title: 'editable-markdown.tsx: hardcoded color/description field names for mentions'
---
**File:** `kanban-app/ui/src/components/editable-markdown.tsx:110-111,125-126`\n\nBuilds color and description maps for entity mentions by hardcoding `\"color\"` and `\"description\"` field names. Should use schema-declared fields for mention styling. #field-special-case