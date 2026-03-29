---
assignees:
- claude-code
position_column: todo
position_ordinal: 8d80
title: Field editors call updateField directly — remove save responsibility from containers
---
## What

Refactor the `EditorProps` interface and all field editors so they receive `entityType`, `entityId`, and `fieldName` props and call `useFieldUpdate().updateField(...)` themselves on every save-worthy exit path. The containers (`FieldRow` in inspector, `renderEditor` in grid-view) stop providing save logic via `onCommit` — they only provide lifecycle callbacks (`onDone`, `onCancel`) that mean "I'm finished editing, close me."

### Files to modify
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — update `EditorProps` interface
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — `FieldPlaceholderEditor` calls `updateField` itself
- `kanban-app/ui/src/components/fields/editors/number-editor.tsx` — same
- `kanban-app/ui/src/components/fields/editors/select-editor.tsx` — same
- `kanban-app/ui/src/components/fields/editors/date-editor.tsx` — same
- `kanban-app/ui/src/components/fields/editors/color-palette-editor.tsx` — same
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — same

### Approach
1. Add `entityType`, `entityId`, `fieldName` to `EditorProps`
2. Each editor calls `useFieldUpdate().updateField(entityType, entityId, fieldName, value)` when it has a value to save
3. `onCommit(value)` becomes `onDone()` — a signal to the container that editing is complete
4. `onCancel()` stays — means "discard and close" (CUA Escape)
5. For vim mode, Escape always saves then signals done — the editor handles this, not the container

## Acceptance Criteria
- [ ] Every editor calls `updateField` directly — no container provides save logic
- [ ] Inspector and grid produce identical save behavior for all field types
- [ ] `onCommit` callback removed from `EditorProps`, replaced with `onDone`
- [ ] Existing field-update-context.test.tsx still passes

## Tests
- [ ] Update `kanban-app/ui/src/components/fields/field-placeholder.test.tsx` — verify `updateField` is called by the editor itself (mock `useFieldUpdate`)
- [ ] Add equivalent tests for NumberEditor, SelectEditor, DateEditor
- [ ] `cd kanban-app/ui && npx vitest run` — all 419+ tests pass