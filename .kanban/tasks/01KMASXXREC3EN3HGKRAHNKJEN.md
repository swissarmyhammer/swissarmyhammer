---
assignees:
- claude-code
depends_on:
- 01KMASVEQA7K7F1TKE3ACAWDXT
- 01KMASWCVYNTTX7GRKPPTCVQAX
position_column: done
position_ordinal: fffffffffff880
title: ColorPaletteEditor — call updateField directly
---
## What

Update `ColorPaletteEditor` to use the new `EditorProps` contract.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/color-palette-editor.tsx`

### Approach
1. Accept `entityType`, `entityId`, `fieldName` from props
2. Call `updateField` on color change (debounced) and popover close
3. Replace `onCommit` with `onDone()`

## Acceptance Criteria
- [ ] Editor calls `updateField` directly
- [ ] Color adapter in `editor-save.test.tsx` passes all keymap × exit combos

## Tests
- [ ] `cd kanban-app/ui && npx vitest run src/components/fields/editors/editor-save.test.tsx` — color rows green