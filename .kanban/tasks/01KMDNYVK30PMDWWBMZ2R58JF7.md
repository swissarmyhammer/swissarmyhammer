---
assignees:
- claude-code
position_column: todo
position_ordinal: cc80
title: 'quick-capture: vim Escape in normal mode should close, Enter should submit'
---
## What

In quick capture with vim keymap: after pressing Escape to exit insert mode, pressing Escape again (in normal mode) should close the quick capture window, and Enter (in normal mode) should submit the task. Currently neither works — the user is stuck in normal mode with no way to submit or dismiss.

### Files to investigate
- `kanban-app/ui/src/components/quick-capture.tsx` — window close / submit logic
- `kanban-app/ui/src/components/fields/text-editor.tsx` — vim mode Escape/Enter handling in CM6
- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — submit/cancel extension builder

## Acceptance Criteria
- [ ] Vim normal mode: Escape closes quick capture window
- [ ] Vim normal mode: Enter submits the task
- [ ] CUA/emacs behavior unchanged

## Tests
- [ ] Manual smoke test in quick capture with vim keymap
- [ ] Zero type errors"