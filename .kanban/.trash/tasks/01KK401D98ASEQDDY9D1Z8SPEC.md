---
position_column: todo
position_ordinal: b9
title: 'SelectEditor component (field.editor: select)'
---
Create `ui/src/components/fields/editors/select-editor.tsx` — a proper select editor that replaces the native `<select>` in the grid's CellEditor.

Currently: grid uses a bare `<select>` element (SelectCellEditor in cell-editor.tsx). Inspector has no select editor at all.

Target: A styled dropdown that works in both compact (grid) and full (inspector) modes. Uses field.type.options for the option list with colors.

- [ ] Create select-editor.tsx with SelectEditor component
- [ ] Support compact mode (inline dropdown for grid cells) and full mode (wider dropdown for inspector)
- [ ] Read options from field.type.options (SelectOption[] with label, value, color)
- [ ] Show colored option badges in the dropdown
- [ ] Auto-focus on mount, commit on selection, cancel on Escape
- [ ] Wire into FieldEditor dispatcher
- [ ] Run tests