---
assignees:
- claude-code
position_column: todo
position_ordinal: c380
title: 'Inspector: clicking a field should set nav cursor and enter edit mode'
---
## What

When a field is clicked in the entity inspector, it enters edit mode visually but the inspector's keyboard cursor (`useInspectorNav`) is not updated. After clicking field 3 and editing, pressing `j`/`k` moves from wherever the cursor was before — not from field 3.

### Root cause

`FieldRow` in `kanban-app/ui/src/components/entity-inspector.tsx` handles `onEdit` (line 166–168) by setting local `editing` state, but never calls `nav.setFocusedIndex(index)` or `nav.enterEdit()`. The inspector nav state and the visual editing state are desynchronized.

### Fix

In `kanban-app/ui/src/components/entity-inspector.tsx`:

1. Add `onFocus?: (index: number) => void` prop to `FieldRowProps`
2. In `handleEdit`, call `onFocus?.(index)` before `setEditing(true)` — this syncs the inspector cursor to the clicked field
3. In the `renderField` closure, pass `onFocus={(idx) => { nav.setFocusedIndex(idx); nav.enterEdit(); }}` with the field's flat index
4. Use a stable callback pattern (ref or single useCallback) to avoid breaking React.memo if FieldRow is ever memoized

### Files to modify

- **Modify**: `kanban-app/ui/src/components/entity-inspector.tsx`

## Acceptance Criteria

- [ ] Clicking a field sets `nav.focusedIndex` to that field's index
- [ ] Clicking a field sets `nav.mode` to `"edit"`
- [ ] After clicking field 3 and pressing Escape, `j`/`k` navigation continues from field 3
- [ ] Keyboard-only flow still works: `j`/`k` to navigate, `i`/`Enter` to edit

## Tests

- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — add test: clicking a field row syncs the inspector nav cursor to that field's index
- [ ] `pnpm vitest run` passes