---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa780
title: Debounced autosave infrastructure
---
## What

Create a `useDebouncedSave` hook and wire it into `Field.tsx` so editors can report intermediate value changes that are auto-persisted with debounce.

### Files to create/modify:
- **Create** `kanban-app/ui/src/lib/use-debounced-save.ts` — new hook
- **Modify** `kanban-app/ui/src/components/fields/editors/index.ts` — add `onChange` to `EditorProps`
- **Modify** `kanban-app/ui/src/components/fields/field.tsx` — add `onChange` to `FieldEditorProps`, wire `useDebouncedSave`, pass `onChange` to editors, flush on commit/cancel

### Hook API:
```ts
useDebouncedSave({
  updateField,    // from useFieldUpdate()
  entityType,
  entityId,
  fieldName,
  delayMs: 1000,  // configurable, default 1s
}) => { onChange: (value: unknown) => void, flush: () => void }
```

- `onChange(value)` — starts/restarts a debounce timer; when it fires, calls `updateField`
- `flush()` — if a save is pending, fires it immediately and clears the timer
- Timer cleanup on unmount

### Field.tsx changes:
- Create debounced save from `useDebouncedSave`
- Pass `onChange` to editor component
- In `handleCommit`: call `flush()` then `onCommit` (flush ensures no stale pending save after the final commit)
- In `handleCancel`: cancel any pending save (don't flush — user is discarding)
- TextEditor already accepts `onChange` — it will work immediately once Field passes it

## Acceptance Criteria
- [ ] `useDebouncedSave` hook exists with onChange/flush API
- [ ] `EditorProps.onChange` is an optional callback
- [ ] `Field.tsx` passes onChange to editors and flushes on commit
- [ ] Pending save is cancelled on unmount and on cancel
- [ ] No behavior change for editors that don't call onChange yet

## Tests
- [ ] `kanban-app/ui/src/lib/use-debounced-save.test.ts` — unit tests: debounce fires after delay, flush fires immediately, cancel prevents firing, unmount cleanup
- [ ] `pnpm --filter kanban-app test` passes