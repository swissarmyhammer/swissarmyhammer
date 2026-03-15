---
assignees:
- claude-code
position_column: done
position_ordinal: fffffb80
title: CM6 vim normal-mode Enter/Escape via Vim.defineAction + mapCommand
---
## What
Replace the `EditorView.domEventHandlers` approach in `cm-submit-cancel.ts` with the canonical vim API: `Vim.defineAction()` + `Vim.mapCommand()`. The current DOM-level interception doesn't reliably fire before vim's internal key dispatch — vim consumes `<CR>` (mapped to `j^`) before our handler sees it.

### Root cause
Vim's ViewPlugin processes keydown events internally. `EditorView.domEventHandlers` fires in parallel with vim's own handlers, and extension ordering determines who wins. Since `keymapExtension(mode)` (which includes `vim()`) is listed before `buildSubmitCancelExtensions()` in the extensions array, vim gets priority and swallows Enter.

### Approach
Use vim's OWN key dispatch to call our callbacks:
1. Register global actions `sahSubmit` and `sahCancel` via `Vim.defineAction()`
2. Map `<CR>` and `<Esc>` in normal-mode context via `Vim.mapCommand()`
3. Route to per-editor callbacks via a `WeakMap<CM5Adapter, {submitRef, cancelRef}>`
4. Register the CM5 adapter in the WeakMap from a `ViewPlugin.define()` (create → register, destroy → delete)
5. For CUA/emacs: keep existing `keymap.of()` approach (works fine)

### Files
- `kanban-app/ui/src/lib/cm-submit-cancel.ts` — rewrite vim branch

## Acceptance Criteria
- [ ] In vim mode, typing text then pressing Escape (→ normal) then Enter fires onSubmitRef
- [ ] In vim mode, typing text then pressing Escape (→ normal) then Escape fires onCancelRef
- [ ] In vim mode, Escape in insert mode still transitions to normal mode (vim handles it)
- [ ] In CUA/emacs mode, Enter fires onSubmitRef, Escape fires onCancelRef (unchanged)
- [ ] No regressions in FieldPlaceholderEditor used in grid cell editing
- [ ] No regressions in command palette Enter/Escape handling

## Tests
- [ ] Manual test: Quick Capture in vim mode — type text, Esc, Enter → task created
- [ ] Manual test: Quick Capture in vim mode — type text, Esc, Esc → window dismissed
- [ ] Manual test: Grid cell edit in vim mode — Enter/Escape still work
- [ ] Manual test: CUA mode — Enter submits, Escape cancels (unchanged)