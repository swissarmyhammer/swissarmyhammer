---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
- 01KMASWCVYNTTX7GRKPPTCVQAX
position_column: done
position_ordinal: fffffffffff680
title: SelectEditor — call updateField directly
---
## What

Update `SelectEditor` to use the new `EditorProps` contract.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/select-editor.tsx`

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `updateField` on selection change
3. Replace `onCommit` with `onDone()`
4. Escape calls `onCancel()` — select is click-to-commit so Escape is always discard

## Acceptance Criteria
- [ ] Editor calls `updateField` directly
- [ ] Select adapter in `editor-save.test.tsx` passes all keymap × exit combos

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — select rows green