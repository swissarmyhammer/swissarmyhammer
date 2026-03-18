---
assignees:
- claude-code
depends_on:
- 01KKS49YPTJEAZYX34ABQ4JVET
position_column: done
position_ordinal: ffffffb880
title: 'Quick Capture: auto-enter vim insert mode on show'
---
## What
When the quick capture window is shown in vim mode, the editor should auto-enter insert mode so the user can start typing immediately. Currently `handleCreateEditor` exits insert mode, which means the user lands in normal mode and has to press `i` first — bad UX for a quick capture popup.

Follow the same pattern as command-palette.tsx: use `Vim.handleKey(cm, "i", "mapping")` with a `requestAnimationFrame` retry loop after mount.

### Files
- `kanban-app/ui/src/components/fields/field-placeholder.tsx` — add auto-insert-mode when `onSubmit` is provided (indicating quick-capture/popup usage)

## Acceptance Criteria
- [ ] Quick capture in vim mode: editor starts in insert mode, user can type immediately
- [ ] Grid cell editing: still starts in normal mode (no onSubmit prop)
- [ ] After typing + Esc → normal mode, Enter/Escape work per card 1

## Tests
- [ ] Manual test: Open quick capture in vim mode → cursor is in insert mode, can type immediately
- [ ] Manual test: Grid cell edit in vim mode → still starts in normal mode