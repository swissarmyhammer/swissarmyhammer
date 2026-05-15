---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe180
title: 'MultiSelectEditor: render selected items as inline CM6 pill widgets'
---
## What

The multi-select editor currently renders selected items as pills in a separate div above the CM6 input. This feels like two disconnected controls. Selected items should render as inline atomic widgets inside CM6 itself — the entire editing surface is one cohesive text editor.

### Current behavior
- Pills in a div above CM6 with × buttons
- CM6 input is empty (for typing new items)
- Backspace in empty CM6 does nothing to pills
- Two disconnected pieces

### Expected behavior
- Selected items render as inline pill widgets inside CM6
- The document content looks like: `[#bug][#feature]|` (where [] are atomic widgets)
- Backspace deletes into the last pill (removes it)
- Typing after the last pill starts a new search/entry
- The whole thing feels like one text input with inline tokens

### Technical approach
- Use CM6 `Decoration.widget` or `Decoration.replace` to render pills inline
- Document model: each selected item is represented as a token in the doc (e.g. `#tag-name `)
- Decorations replace the raw text with styled pill widgets
- Backspace at the start of a token removes the whole token (atomic)
- New text after the last token triggers autocomplete

### Files to modify
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — rewrite to use inline CM6 decorations instead of external pill div

## Acceptance Criteria
- [ ] Selected items render as inline pill widgets inside CM6
- [ ] Backspace removes the last pill when cursor is at its boundary
- [ ] Typing new text triggers autocomplete as before
- [ ] Enter/Escape commit/cancel behavior unchanged
- [ ] Visual style matches current pill appearance

## Tests
- [ ] Zero type errors
- [ ] Manual smoke test"