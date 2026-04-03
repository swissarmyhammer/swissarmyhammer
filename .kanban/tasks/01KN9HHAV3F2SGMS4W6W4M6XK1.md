---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb480
title: 'Fix: date-editor Escape — replace ad-hoc domEventHandlers with buildSubmitCancelExtensions'
---
## What

In `kanban-app/ui/src/components/fields/editors/date-editor.tsx` (lines 111–157), the extension array uses a hand-rolled `EditorView.domEventHandlers` for Escape/Enter in vim mode and a separate `keymap.of` for CUA/emacs — registered after the vim extension. Same root cause as the command palette bug: vim consumes Escape before the handler fires.

**Fix**: Replace the entire vim/CUA branching block (lines 116–157) with `buildSubmitCancelExtensions`. The date-editor's contract:
- **onSubmit** (Enter) → commit resolved date or cancel
- **onCancel** (Escape) → vim: commit if resolved else cancel; CUA/emacs: cancel
- `singleLine: true`

**Files to modify**:
- `kanban-app/ui/src/components/fields/editors/date-editor.tsx` — replace lines 116–157 with `buildSubmitCancelExtensions`

## Acceptance Criteria
- [ ] In vim mode, Escape in normal mode commits the date (if resolved) or cancels
- [ ] In vim mode, Escape in insert mode exits to normal mode (does not commit/cancel)
- [ ] CUA/emacs Escape cancels (unchanged)
- [ ] Enter commits in all modes (unchanged)

## Tests
- [ ] Add or update date-editor test: vim normal-mode Escape commits resolved date
- [ ] Add or update test: vim insert-mode Escape does NOT commit
- [ ] Run `pnpm --filter kanban-app-ui test` — all tests pass