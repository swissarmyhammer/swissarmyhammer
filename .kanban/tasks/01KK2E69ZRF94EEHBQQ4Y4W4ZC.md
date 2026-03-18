---
position_column: done
position_ordinal: ffffc880
title: useGrid hook — cursor navigation and mode
---
Build the core `useGrid` hook managing cursor position (row, col), normal/edit mode, and visual selection. Pure state logic with tests.

**Create `ui/src/hooks/use-grid.ts`:**
- [ ] State: `cursor: {row, col}`, `mode: "normal" | "edit"`, `selection: Set<number>`, `rowCount`, `colCount`
- [ ] Navigation: `moveUp/Down/Left/Right` with bounds clamping
- [ ] Jumps: `moveToTop` (gg), `moveToBottom` (G), `moveToFirstCol` (0), `moveToLastCol` ($)
- [ ] Mode: `enterEdit()`, `exitEdit()`
- [ ] Selection: `toggleSelection()`, `selectDown()`, `selectUp()`
- [ ] All functions memoized with useCallback
- [ ] Hook takes `{rowCount, colCount}` and returns cursor, mode, selection, and all functions

**Create `ui/src/hooks/use-grid.test.ts`:**
- [ ] Tests for bounds clamping (empty grid, single row/col, at edges)
- [ ] Tests for mode transitions
- [ ] Tests for selection toggle/extend
- [ ] `npm test` passes