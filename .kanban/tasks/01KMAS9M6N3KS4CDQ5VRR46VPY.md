---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffa480
title: 'multi-select-editor.tsx: hardcoded parse-body-tags detection'
---
**File:** `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx:53`\n\n```ts\nconst isComputedTags = field.type.kind === \"computed\" && field.type.derive === \"parse-body-tags\";\n```\n\nHardcodes detection of computed tag fields to determine target entity type, commit format, prefix, and display field. This info should come from the field definition — computed fields that are editable should declare their target entity type the same way reference fields do. #field-special-case