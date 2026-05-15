---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9d80
title: 'EditorProps and FieldEditorProps: onChange declared in two places with same signature'
---
**File:** `kanban-app/ui/src/components/fields/editors/index.ts` and `kanban-app/ui/src/components/fields/field.tsx`\n**Severity:** nit\n\n`onChange?: (value: unknown) => void` is added to both `EditorProps` (editors/index.ts) and `FieldEditorProps` (field.tsx). These are parallel interfaces that should ideally share the definition or one should extend the other. Currently they are in sync but could drift. Not blocking since the pattern already existed for `onCommit`/`onCancel`, but worth noting for future cleanup. #review-finding