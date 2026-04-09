---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa280
title: 'Bug: Filter formula bar does not save on vim insert→normal (Escape)'
---
## What

The perspective filter formula bar does not persist the filter when the user
presses Escape to return from vim insert mode to normal mode. It **does** save
correctly when the user presses Enter in normal mode.

### Root cause

The save path for vim insert→normal Escape flows through:

1. `buildVimEscapeExtension` bubble phase → `saveInPlaceRef`
   (`kanban-app/ui/src/lib/cm-submit-cancel.ts`)
2. `saveInPlace` in `useExitActions` → calls `refs.onChangeRef.current?.(text)`
   (`kanban-app/ui/src/components/fields/text-editor.tsx`)
3. FilterEditor's `handleChange` only does `setError(null)` — it **never
   dispatches** `perspective.filter`
   (`kanban-app/ui/src/components/filter-editor.tsx`)

By contrast, Enter in normal mode goes through `onSubmitRef` → `semanticSubmitRef`
→ `commitAndExit` → `handleCommit` → dispatches the filter command. This path
works correctly.

### Fix approach

The FilterEditor needs to commit the filter on the `onChange` path (used by
`saveInPlace`) when the text represents a valid, changed filter — not just
clear the error state. Modify `useFilterDispatch` so that `handleChange`
validates and dispatches the filter when the value has actually changed
(compare against the incoming `filter` prop). Alternatively, add a dedicated
`onSaveInPlace` callback to TextEditor's props so FilterEditor can distinguish
between keystroke-level changes and vim save-in-place events.

The cleaner approach is to add a wrapper `onBlur` handler on FilterEditor's
container div (matching the pattern in `InlineRenameEditor`) that tracks
the latest text via `onChange` + a ref, then commits on blur. However, the
filter bar is persistent (`repeatable: true`) and should not commit on every
blur — only on intentional save gestures. So the preferred fix is to make
`saveInPlace` in `useExitActions` call `onCommitRef` (not `onChangeRef`) for
repeatable editors, since vim insert→normal is semantically a "save" gesture.

### Files to modify

- `kanban-app/ui/src/components/fields/text-editor.tsx` — `useExitActions`
  `saveInPlace` callback: for repeatable editors, call `onCommitRef` instead of
  `onChangeRef` so the filter bar dispatches on vim insert→normal.
- `kanban-app/ui/src/components/filter-editor.tsx` — verify the fix works with
  the repeatable commit path (no code change expected, but verify).

## Acceptance Criteria

- [ ] In vim mode, typing a filter expression and pressing Escape (insert→normal)
      persists the filter via `perspective.filter` command dispatch
- [ ] Enter in normal mode continues to work as before
- [ ] Escape from normal mode (cancel) continues to work (commits in vim mode)
- [ ] Non-vim modes (CUA/emacs) are unaffected
- [ ] Field editors (non-repeatable) are unaffected — `saveInPlace` still calls
      `onChange` for them
- [ ] The `repeatable` flag gates the new behavior

## Tests

- [ ] `kanban-app/ui/src/lib/cm-submit-cancel.test.ts` — add test verifying
      that vim Escape from insert mode fires `saveInPlaceRef`
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — add test that
      simulating vim insert→normal Escape dispatches `perspective.filter`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx src/lib/cm-submit-cancel.test.ts`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.