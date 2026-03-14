---
position_column: todo
position_ordinal: c3
title: 'NumberEditor component (field.editor: number)'
---
Create `ui/src/components/fields/editors/number-editor.tsx` — unified number editor. Native number input is acceptable here (CM6 doesn't help with number stepping/validation).

Currently: Grid uses native `<input type="number">`. Inspector uses FieldPlaceholder (CM6 text edit).

Target: A styled number input with min/max from field.type config.

- [ ] Create number-editor.tsx with NumberEditor component
- [ ] Read min/max from field.type properties if available
- [ ] Styled input, right-aligned, tabular-nums
- [ ] Auto-focus on mount, commit on blur/Enter, cancel on Escape
- [ ] Wire into FieldEditor dispatcher
- [ ] Run tests