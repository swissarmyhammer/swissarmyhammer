---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffe080
title: 'WARNING: AttachmentEditor accesses field.type.multiple with unsafe cast'
---
**File**: kanban-app/ui/src/components/fields/editors/attachment-editor.tsx (isMultiple function)\n\n**What**: The `isMultiple` function accesses `field.type.multiple` via `(type as unknown as { multiple: boolean }).multiple`. This is the same pattern prohibited by the JS/TS review guidelines.\n\n**Why**: Like the multi-select editor, `multiple` should be a top-level FieldDef property populated by the backend.\n\n**Suggestion**: Add `multiple` to FieldDef and have the backend populate it from the field type config.\n\n**Subtasks**:\n- [ ] Add multiple as a top-level FieldDef property\n- [ ] Update AttachmentEditor to read field.multiple directly\n- [ ] Verify fix by running tests #review-finding