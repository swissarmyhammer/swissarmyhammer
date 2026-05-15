---
assignees:
- claude-code
attachments: []
depends_on:
- 01KN2PQJH0EQAH06DNABFD0095
position_column: done
position_ordinal: ffffffffffffffffff9180
title: Wire onChange into all editors
---
## What

Add `onChange` calls to every editor so intermediate values are reported to Field's debounced autosave. TextEditor already supports `onChange` — the remaining editors need it added.

### Files to modify:

1. **`kanban-app/ui/src/components/fields/text-editor.tsx`** — No changes needed. Already accepts `onChange` prop and fires it via CM6 `updateListener`. Field.tsx (card 1) will pass onChange to it.

2. **`kanban-app/ui/src/components/fields/editors/number-editor.tsx`** — Accept `onChange` from `EditorProps`. Call `onChange(numericValue)` inside the existing `setDraft` handler.

3. **`kanban-app/ui/src/components/fields/editors/date-editor.tsx`** — Accept `onChange`. Call it when the date string changes in the CodeMirror input.

4. **`kanban-app/ui/src/components/fields/editors/color-palette-editor.tsx`** — Accept `onChange`. Call it on every color picker change event (already has a draft state update).

5. **`kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx`** — Accept `onChange`. Call it when items are added/removed from the selection.

6. **`kanban-app/ui/src/components/fields/editors/select-editor.tsx`** — Accept `onChange`. Call it in `onValueChange`. (Note: select already commits on selection in most cases, so autosave is less critical here but should be consistent.)

### Pattern for each editor:
```tsx
// In the onChange/setDraft handler:
setDraft(newVal);
onAutoSave?.(newVal);  // or onChange?.() depending on final prop name
```

## Acceptance Criteria
- [ ] All 6 editors call `onChange` when their draft value changes
- [ ] Editors that don't receive `onChange` (prop is optional) continue to work identically
- [ ] TextEditor works with autosave via its existing onChange → Field onChange path
- [ ] The editor-save test matrix still passes (commit-on-exit behavior unchanged)

## Tests
- [ ] Update `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — add test cases verifying onChange is called during typing/interaction (before commit)
- [ ] `pnpm --filter kanban-app test` passes