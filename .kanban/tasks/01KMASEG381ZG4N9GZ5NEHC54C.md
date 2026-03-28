---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffff8b80
title: 'mention-pill.tsx: hardcoded field name fallback chain'
---
**File:** `kanban-app/ui/src/components/mention-pill.tsx:55,70-73`\n\nHardcodes field name search order `[\"tag_name\", \"name\", \"title\", \"id\"]` for display name resolution, and a separate fallback chain `getStr(entity, \"title\") || getStr(entity, \"name\") || getStr(entity, \"tag_name\")` for tooltips.\n\nEntity definitions already declare `mention_display_field` — that should be the only source of truth for which field to display in mention pills. #field-special-case #te