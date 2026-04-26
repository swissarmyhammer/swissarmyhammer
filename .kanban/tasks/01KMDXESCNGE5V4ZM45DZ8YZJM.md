---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe080
title: 'vim: exiting insert mode on a field should save and close the editor'
---
## What

On inline field editing (cards, grid, inspector), pressing Escape in vim insert mode should save the field and close the editor in one step. Currently it takes two Escapes: first exits to normal mode, second commits. The user expects one Escape = done.

This is different from quick-capture where normal mode is a useful intermediate state. For inline field editing, there's no reason to stay in normal mode — the field is a single-line value, not a document.

### Current behavior
1. Click field → insert mode
2. Escape → normal mode (field still open)
3. Escape → commit and close

### Expected behavior
1. Click field → insert mode
2. Escape → save and close

### Files to modify
- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — vim insert-mode Escape handler should commit+close, not just save-in-place
- `kanban-app/ui/src/components/fields/text-editor.tsx` — saveInPlaceRef wiring

## Acceptance Criteria
- [ ] Vim insert Escape on inline fields: saves and closes editor
- [ ] Quick-capture behavior unchanged (insert Escape → normal mode)
- [ ] CUA/emacs behavior unchanged
- [ ] No regressions in grid/inspector

## Tests
- [ ] Manual smoke test with all keymaps
- [ ] Zero type errors"