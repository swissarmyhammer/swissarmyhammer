---
position_column: done
position_ordinal: ffff9780
title: Cell editors + edit mode
---
Implement cell editing flow: grid.edit (i/Enter) mounts editor in focused cell, commit persists value, returns to normal mode.

**Create cell editor components in `ui/src/components/cells/`:**
- [ ] `cell-editor-markdown.tsx` — CM6 inline editor, reuse pattern from FieldPlaceholderEditor. Single/multi-line from field def. Vim/CUA/Emacs keymaps from useKeymap().
- [ ] `cell-editor-select.tsx` — Radix Popover/DropdownMenu for select fields. Shows options, single click commits.
- [ ] `cell-editor-multi-select.tsx` — Checkbox list popover for multi-select. Toggle options, Enter/Escape commits.
- [ ] `cell-editor-date.tsx` — Native `<input type="date">` initially.
- [ ] `cell-editor-color.tsx` — Reuse ColorField pattern from entity-inspector.
- [ ] `cell-editor-number.tsx` — Numeric input with min/max from field def.
- [ ] `cell-editor-dispatch.tsx` — Routes editor type to correct component. Computed fields (editor=none) not editable.

**Update `ui/src/components/data-table.tsx`:**
- [ ] When `mode === "edit"` and cell matches cursor, render editor instead of display
- [ ] Editor receives `value`, `field`, `onCommit`, `onCancel` props

**Update `ui/src/components/grid-view.tsx`:**
- [ ] `onCellCommit` callback persists via `useFieldUpdate().updateField()`
- [ ] After commit/cancel, `exitEdit()` returns grid to normal mode

- [ ] i/Enter opens appropriate editor
- [ ] Escape exits edit mode
- [ ] Value persisted via updateField Tauri command
- [ ] Computed fields skip editing