---
assignees:
- claude-code
position_column: todo
position_ordinal: c680
title: 'MultiSelectEditor: typing #tag then space should convert to inline pill immediately'
---
## What

When typing `#one ` (tag followed by space) in the multi-select editor, the text stays as plain text in CM6. Expected: the text converts to an inline pill immediately on space, like a token input.

### Current behavior
- Type `#one ` — stays as text `#one ` in CM6
- Tags only become pills on commit (Enter/Escape/blur)

### Expected behavior
- Type `#one` then space — `#one` converts to a pill inline, cursor is ready for the next tag
- The editor works like a token input: type, space to confirm, type next

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — detect space after a valid tag and convert to pill

## Acceptance Criteria
- [ ] Typing `#tagname ` (space after tag) converts the tag text to a pill
- [ ] Cursor remains in the editor ready for next input
- [ ] Multiple tags can be entered in sequence: `#one #two #three`

## Tests
- [ ] Add test: type `#bug `, assert pill appears and CM6 input is empty