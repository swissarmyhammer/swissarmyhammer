---
position_column: todo
position_ordinal: c1
title: 'ColorPaletteEditor component (field.editor: color-palette)'
---
Create `ui/src/components/fields/editors/color-palette-editor.tsx` — unified color editor replacing the native `<input type=color>` in grid and the HexColorPicker in inspector.

Currently: Grid uses native `<input type="color">`. Inspector uses react-colorful's HexColorPicker wrapped in ColorField.

Target: A single component with mode parameter. Compact mode shows a small popover palette. Full mode shows the HexColorPicker inline.

- [ ] Create color-palette-editor.tsx with ColorPaletteEditor component
- [ ] Compact mode: small color swatch grid popover (preset colors + custom hex input)
- [ ] Full mode: reuse existing HexColorPicker from react-colorful (already a dependency)
- [ ] Auto-focus on mount, commit on selection, cancel on Escape
- [ ] Wire into FieldEditor dispatcher
- [ ] Run tests