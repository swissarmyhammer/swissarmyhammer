---
position_column: todo
position_ordinal: c0
title: 'MultiSelectEditor component (field.editor: multi-select)'
---
Create `ui/src/components/fields/editors/multi-select-editor.tsx` — editor for multi-select fields (assignees, depends_on, attachments).

Currently: No multi-select editor exists in either grid or inspector. These fields are read-only or edited only through commands.

Target: A checkbox/tag picker that works in both compact and full modes. For reference fields, resolves entity names from EntityStore.

- [ ] Create multi-select-editor.tsx with MultiSelectEditor component
- [ ] For reference fields (entity in field.type), look up available entities from EntityStore
- [ ] Show checkboxes or toggleable pills for each option
- [ ] Compact mode: popover dropdown; full mode: inline list
- [ ] Commit on change (not on close), cancel on Escape
- [ ] Wire into FieldEditor dispatcher
- [ ] Run tests