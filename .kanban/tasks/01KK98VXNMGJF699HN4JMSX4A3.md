---
position_column: done
position_ordinal: ffff9680
title: Fix tag and assignee editors to use CM6
---
Two broken editors in the grid:

1. **Tags field** (`kind: computed, editor: none`): `resolveEditor` falls through to markdown, giving a FieldPlaceholderEditor with no tag autocomplete. Typing `#test` shows "no word under cursor" and doesn't save (computed field).

Fix: Add `tag-select` editor type. CM6-based multi-select that:
- Shows current tags as pills
- CM6 autocomplete from available tags (via `search_mentions`)
- On select: calls `tag task` command (appends `#tag` to body)
- On remove: calls `untag task` command (removes `#tag` from body)
- `resolveEditor` returns `"tag-select"` for computed+parse-body-tags

2. **Assignees field** (`kind: reference, editor: multi-select`): Uses plain `<input type="text">` — MUST use CM6 per project rules.

Fix: Rewrite MultiSelectEditor to use CM6 with autocomplete.

- [ ] Create tag-select editor component using CM6
- [ ] Wire resolveEditor for computed parse-body-tags fields
- [ ] Rewrite MultiSelectEditor with CM6 instead of input
- [ ] Wire into CellEditor dispatch
- [ ] Test both editors in grid