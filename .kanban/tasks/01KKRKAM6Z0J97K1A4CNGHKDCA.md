---
assignees:
- claude-code
depends_on:
- 01KKRKA4D87K7VM73RN6A1FV2V
position_column: done
position_ordinal: ffffffec80
title: 'Grid cell editing: refactor to use onSubmit/onCancel from CM6 editor'
---
## What

Refactor the grid view's cell editing to use the new `onSubmit`/`onCancel` semantic callbacks from the CM6 editor, replacing any raw key handling in `grid-view.tsx` that duplicates what the editor now handles internally.

**Current state:**
- `grid-view.tsx` (line 86-91) has a keyboard handler that catches Escape during edit mode to call `g.exitEdit()` — but this is at the window level, not inside the CM6 editor
- `CellEditor` dispatches to various editors (`MarkdownEditor`, `SelectEditor`, etc.) that each have their own `onCommit`/`onCancel` props
- `MarkdownEditor` in compact mode uses `FieldPlaceholderEditor` which will now have `onSubmit`/`onCancel`

**Approach:**
- Wire `onSubmit` through `CellEditor` → `MarkdownEditor` → `FieldPlaceholderEditor` so that Enter-to-submit works cleanly from the CM6 editor
- The grid-level Escape handler for edit mode (line 86-91) can remain as a fallback for non-CM6 editors (select, date, number), but the CM6 editors should drive commit/cancel through their own callbacks
- Verify that the existing `onCommit`/`onCancel` flow from `renderEditor` in `grid-view.tsx` (line 262-278) correctly chains through to `exitEdit()`

**Affected files:**
- `kanban-app/ui/src/components/grid-view.tsx` — `renderEditor` callback (lines 262-278)
- `kanban-app/ui/src/components/cells/cell-editor.tsx` — pass through `onSubmit`/`onCancel` to `MarkdownEditor`
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — pass through to `FieldPlaceholderEditor`

## Acceptance Criteria
- [ ] In grid edit mode with vim keymap: Escape in insert mode → normal mode (stays in edit), Escape in normal mode → cancel (exits edit)
- [ ] In grid edit mode with CUA keymap: Escape → cancel, Enter → submit
- [ ] Grid cell editing for non-text fields (select, date, number, color) continues to work unchanged
- [ ] No duplicate key handling between grid-view's window-level handler and the CM6 editor

## Tests
- [ ] Manual test: grid view — enter edit mode, type text, press Enter → submits and exits edit mode
- [ ] Manual test: grid view — enter edit mode, press Escape (vim normal) → cancels and exits
- [ ] Manual test: grid view — select/date/color editors still work correctly
- [ ] `npm run typecheck` passes in `kanban-app/ui/`