---
assignees:
- claude-code
position_column: todo
position_ordinal: c580
title: 'MultiSelectEditor: backspace should delete into pills — pills feel disconnected from editor'
---
## What

When editing a multi-select field (e.g. tags), selected items render as pills above the CM6 input. The pills have × buttons but backspace in the empty CM6 input does not delete the last pill. This feels broken — the pills appear disconnected from the editor.

### Current behavior
- Pills render in a separate div above CM6
- CM6 input is empty (for typing new items)
- Backspace in empty CM6 does nothing to the pills
- User must click × on each pill to remove it

### Expected behavior
- Backspace in empty CM6 input removes the last pill (like a tag input)
- The editor feels like one cohesive control, not two disconnected pieces

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — add backspace handler to remove last selected item when CM6 input is empty

## Acceptance Criteria
- [ ] Backspace in empty CM6 input removes the last selected item
- [ ] Backspace with text in CM6 input behaves normally (deletes text)
- [ ] × button on pills still works

## Tests
- [ ] Add test: render with 2 pills, press backspace in empty editor, assert 1 pill remains