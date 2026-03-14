---
position_column: todo
position_ordinal: b8
title: Extract shared field editor components into fields/ directory
---
Move editor components out of cell-editor.tsx (and inspector inline code) into `fields/editors/` so both grid and inspector import the same editor. Dispatch on `field.editor` (from YAML), not `field.type.kind`.

What exists today:
- `cell-editor.tsx` has: SelectCellEditor (native select), InputCellEditor (native input for number/date/color), FieldPlaceholderEditor (CM6 text)
- Inspector has: EditableMarkdown (CM6 markdown), ColorField (palette+picker), FieldPlaceholder (CM6 fallback)

Refactor — one component per `field.editor` value:
- [ ] `editors/markdown-editor.tsx` — CM6 via FieldPlaceholderEditor (compact/single-line) or EditableMarkdown (full/multi-line). NEVER plain input.
- [ ] `editors/select-editor.tsx` — extract SelectCellEditor, works in both modes
- [ ] `editors/color-palette-editor.tsx` — extract ColorField from inspector, compact mode shows smaller palette
- [ ] `editors/number-editor.tsx` — styled number input (native input is fine here)
- [ ] `editors/date-editor.tsx` — styled date input (native input is fine here)
- [ ] Refactor CellEditor to dispatch on `field.editor` using new components with mode="compact"
- [ ] Refactor inspector FieldDispatch edit paths to use same components with mode="full"
- [ ] Delete old inline editor code from cell-editor.tsx and entity-inspector.tsx
- [ ] Run tests