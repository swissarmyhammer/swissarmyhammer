---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9280
title: 'multi-select-editor.tsx: hardcoded "color" field name and "tag_name"/"name" fallback'
---
**File:** `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx:70,255`\n\n```ts\nmentionConfig?.displayField ?? (isComputedTags ? \"tag_name\" : \"name\")\nconst color = ent ? getStr(ent, \"color\", \"888888\") : \"888888\";\n```\n\nTwo issues beyond the already-carded parse-body-tags detection:\n1. Line 70: Falls back to hardcoded `\"tag_name\"` or `\"name\"` for display field\n2. Line 255: Hardcodes `\"color\"` field name for entity color lookup in dropdown items #field-special-case