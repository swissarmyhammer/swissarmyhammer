---
assignees:
- claude-code
position_column: todo
position_ordinal: 9f80
title: Make FilterEditor autosave and match field editing vim/keymap behavior
---
## What

The FilterEditor's commit/cancel behavior doesn't match how regular fields work, creating an inconsistent editing experience. Three specific gaps:

1. **Vim Esc (insert→normal) doesn't save**: Fields use `saveInPlaceRef` in `buildSubmitCancelExtensions` to auto-commit when vim exits insert mode. The filter editor doesn't wire `saveInPlaceRef`, so pressing Esc in vim mode does nothing — the filter is lost.

2. **No autosave on change**: Fields auto-dispatch on every keystroke. The filter editor only saves on explicit Enter. It should auto-apply the filter expression after each edit (debounced), showing inline errors for invalid expressions without blocking typing.

3. **`committedRef` guard prevents re-editing**: The filter editor uses a `committedRef` that prevents any subsequent saves after the first commit. Fields don't have this — they allow repeated edits. Remove the committed guard entirely since autosave replaces the one-shot commit model.

### Files to modify

- `kanban-app/ui/src/components/filter-editor.tsx`:
  - **`useFilterCommit`** (lines 110-158): Remove `committedRef` guard. Replace `handleSubmit` with a `saveFilter` function that validates and dispatches (or dispatches clearFilter for empty). Add a debounced `autoSave` that calls `saveFilter` after a short delay (e.g. 300ms) on each doc change.
  - **`useFilterExtensions`** (lines 166-219): Wire `saveInPlaceRef` pointing to `saveFilter` in the `buildSubmitCancelExtensions` call. This makes vim Esc (insert→normal) trigger a save-in-place, matching field behavior. The `changeExtension` listener should call the debounced autoSave instead of just clearing errors.
  - **`FilterEditor` JSX**: Remove the `onClose` callback from save flow — autosave means the editor stays open. The popover's own close mechanism (clicking away, explicit close button) handles dismissal. Keep `onClose` prop for the Clear button.

### Behavioral summary after changes

| Action | CUA mode | Vim mode |
|--------|----------|----------|
| Typing | Autosave (debounced) | Autosave (debounced, insert mode) |
| Enter | Save + close popover | Save (normal mode Enter) |
| Esc | Close popover (discard unsaved) | Insert→normal: save-in-place. Normal Esc: close popover |
| Clear button | Clear filter + close | Clear filter + close |
| Invalid input | Show error inline, don't dispatch | Same |

### Pattern reference
- `text-editor.tsx` lines 194-202: `saveInPlace` callback + `saveInPlaceRef` wiring
- `cm-submit-cancel.ts` lines 162-167: vim Esc capture handler that calls `saveInPlaceRef`

## Acceptance Criteria
- [ ] Vim: Esc from insert mode saves the current filter expression (save-in-place)
- [ ] Typing in the filter editor auto-applies the filter after a debounce delay
- [ ] Invalid expressions show inline error text but don't prevent further typing
- [ ] Valid expression changes dispatch `perspective.filter` command automatically
- [ ] Empty filter auto-dispatches `perspective.clearFilter`
- [ ] Enter still saves + closes the popover (both modes)
- [ ] Clear button still works (clears filter + closes)
- [ ] No `committedRef` guard — re-editing after save works

## Tests
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — update existing tests for autosave model (no explicit Enter required for save)
- [ ] New test: typing valid expression triggers dispatch after debounce
- [ ] New test: typing invalid expression shows error, does not dispatch
- [ ] `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.