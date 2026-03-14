---
position_column: done
position_ordinal: ffed80
title: 'Shared editor components: deduplicate grid CellEditor and inspector FieldDispatch edit paths'
---
The problem: The grid view's CellEditor dispatches on `field.type.kind` (should use `field.editor`). The inspector's FieldDispatch inlines its own editors. Neither shares editor components with the other.

Current duplication:
- **markdown**: grid uses FieldPlaceholderEditor (CM6), inspector uses EditableMarkdown (CM6) — different CM6 wrappers for same purpose
- **select**: grid has SelectCellEditor (native select), inspector has nothing
- **color-palette**: grid has native `<input type=color>`, inspector has ColorField (palette+HexColorPicker)
- **number**: grid has native `<input type=number>`, inspector uses FieldPlaceholder
- **date**: grid has native `<input type=date>`, inspector uses FieldPlaceholder

Fix: Create shared editor components in `fields/editors/` that both CellEditor and FieldDispatch import. Dispatch on `field.editor`. Each accepts `mode: "compact" | "full"`. All text editing uses CM6.

Files to modify:
- `cells/cell-editor.tsx` — dispatch on `field.editor`, import shared editors, pass mode="compact"
- `entity-inspector.tsx` FieldDispatch edit paths — import shared editors, pass mode="full"

Files to create in `fields/editors/`:
- `markdown-editor.tsx` — compact: FieldPlaceholderEditor (CM6 single-line); full: EditableMarkdown (CM6 multi-line with tags)
- `select-editor.tsx` — extract SelectCellEditor, works in both modes
- `color-palette-editor.tsx` — extract ColorField from inspector, compact mode uses smaller palette
- `number-editor.tsx` — styled number input (native input fine here)
- `date-editor.tsx` — styled date input (native input fine here)

- [ ] Create fields/editors/ directory with shared editor components
- [ ] Refactor CellEditor to dispatch on `field.editor` and use shared editors with mode="compact"
- [ ] Refactor inspector FieldDispatch edit paths to use shared editors with mode="full"
- [ ] Delete inline editor code from cell-editor.tsx and entity-inspector.tsx
- [ ] Run tests