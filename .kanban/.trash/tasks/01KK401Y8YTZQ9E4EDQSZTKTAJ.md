---
position_column: todo
position_ordinal: c2
title: 'DateEditor component (field.editor: date)'
---
Create `ui/src/components/fields/editors/date-editor.tsx` — unified date editor replacing the native `<input type=date>` in grid. Inspector currently falls back to FieldPlaceholder for dates.

Currently: Grid uses native `<input type="date">`. Inspector uses FieldPlaceholder (plain text edit of date string).

Target: A styled date input that works in both compact and full modes. Native date input is acceptable here (CM6 doesn't help with date picking).

- [ ] Create date-editor.tsx with DateEditor component
- [ ] Styled date input (not raw native appearance)
- [ ] Auto-focus on mount, commit on blur/Enter, cancel on Escape
- [ ] Wire into FieldEditor dispatcher
- [ ] Run tests