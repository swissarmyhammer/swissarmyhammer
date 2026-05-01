---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffde80
title: 'quick-capture: vim Escape in normal mode should close, Enter should submit'
---
## What

In quick capture with vim keymap: after pressing Escape to exit insert mode, pressing Escape again (in normal mode) should close the quick capture window, and Enter (in normal mode) should submit the task. Currently neither works — the user is stuck in normal mode.

### Behavior matrix

| Keymap | Key | Insert mode | Normal mode |
|--------|-----|-------------|-------------|
| vim | Escape | exit to normal | close window |
| vim | Enter | newline (if multiline) or submit | submit task |
| cua/emacs | Escape | close window | n/a |
| cua/emacs | Enter | submit task | n/a |

### Files to investigate
- `kanban-app/ui/src/components/quick-capture.tsx` — window close / submit logic
- `kanban-app/ui/src/components/fields/text-editor.tsx` — vim mode Escape/Enter handling in CM6
- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — submit/cancel extension builder

## Acceptance Criteria
- [ ] Vim normal mode: Escape closes quick capture window
- [ ] Vim normal mode: Enter submits the task
- [ ] Vim insert mode: Escape exits to normal mode (default vim behavior)
- [ ] CUA/emacs: Escape closes window, Enter submits
- [ ] No regressions in grid/inspector text editing

## Tests
- [ ] Manual smoke test in quick capture with all keymaps
- [ ] Zero type errors"