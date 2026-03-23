---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
title: 'MultiSelectEditor: space between tags creates one slugified tag instead of two'
---
## What

When typing `#one #two` in the multi-select tag editor, it slugifies the entire text as one tag (`one-two`) instead of recognizing two separate tags (`one` and `two`).

### Files to investigate
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — the `commit()` function processes remaining text

### Expected behavior
- `#one #two` should produce two tags: `one` and `two`
- Space (or the prefix character) should be a delimiter between tags

## Acceptance Criteria
- [ ] Typing `#one #two` and committing produces two separate tags
- [ ] Single tags still work: `#bugfix` → `bugfix`

## Tests
- [ ] Add test case for multi-tag input