---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffee80
project: spatial-nav
title: 'Layout: board columns don''t fill the available vertical space'
---
## What

In the board view, each column's card area visually occupies only the bottom portion of the space between the column header and the bottom of the viewport — reported by user: "columns are not 'filling' the column, it's like just the bottom half of the visible column space."

This is a pure layout / CSS bug, unrelated to spatial navigation. It does affect spatial nav indirectly because a short column area means fewer visible card rects for the beam test, so nav below visible range scrolls less smoothly than it should.

### Reproduction

1. Open a board with 3+ cards per column.
2. Observe that the card list is bottom-aligned within the column — empty space above the first card, no empty space below the last card.
3. Compare to the expected behavior: card list is top-aligned, cards stack from the column header downward, empty space is below.

### Root cause

`ColumnHeader` in `kanban-app/ui/src/components/column-view.tsx` wrapped its single header row in a redundant `<div className="flex flex-col min-h-0 min-w-0 flex-1">`. Because that wrapper carried `flex-1`, it became a flex sibling of the card list (`VirtualizedCardList`, also `flex-1`) inside the column's `flex flex-col` scope. The two `flex-1` siblings split the column's vertical space 50/50, leaving the card list pinned to the bottom half.

The wrapper was vestigial from commit 6684a43e3 — before ColumnHeader was extracted into its own function, the enclosing div contained both the header and the card list, so `flex-1` on it made sense. After the extraction the wrapper served no purpose and caused the split.

### Fix

Remove the `flex-col flex-1` wrapper entirely; let the `.column-header-focus` row be a direct child of the column's FocusScope container. It sizes to its content height, and the card list's `flex-1` then claims all remaining vertical space.

### Acceptance

- [x] Failing test that asserts top-alignment of the first card in a column
- [x] Fix: adjust the flex/layout class so cards start at the top of the column body
- [x] Manual verification that the board looks right in the running app
- [x] No regression in column header position or drag-and-drop drop zones

### Tests

Added `describe("ColumnView layout")` in `kanban-app/ui/src/components/column-view.test.tsx`:
- `card list is the only flex-1 child of the column scope` — asserts structural invariant
- `column header row is a direct child of the column scope` — confirms the wrapper is gone

Both tests fail against the old layout and pass against the fix. Full UI test suite (1268 tests) passes.