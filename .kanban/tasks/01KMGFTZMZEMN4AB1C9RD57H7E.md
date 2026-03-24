---
assignees:
- claude-code
depends_on:
- 01KMGFTK8C06Z59GCMVX3KQHG3
position_column: todo
position_ordinal: '888180'
title: Board cursor click-to-focus and mouse integration
---
## What

Wire mouse clicks on board cards to set the keyboard cursor, so mouse and keyboard navigation stay in sync. Also ensure the existing click-to-inspect behavior coexists with the new cursor.

### Changes to `kanban-app/ui/src/components/board-view.tsx`

1. Add an `onCardClick` callback to `ColumnView` that receives `(columnIndex, cardIndex)`
2. On single-click: set the board cursor to that position (`setCursor(colIdx, cardIdx)`)  
3. On double-click: open inspector (existing behavior via the info button, but now also available via double-click on the card body)
4. Click on empty area of the board (outside cards) sets focus to column header level (`card: -1`)

### Changes to `kanban-app/ui/src/components/column-view.tsx`

1. Add `onCardClick?: (cardIndex: number) => void` prop
2. Add `onHeaderClick?: () => void` prop
3. Wire click handlers on card wrappers and column header

### Keyboard-mouse handoff

When the user clicks a card, the cursor jumps there. When the user starts typing navigation keys, the cursor moves from wherever it was last set (whether by click or keyboard). This is the same behavior as grid-view's `handleCellClick`.

## Acceptance Criteria

- [ ] Clicking a card sets the board cursor to that card
- [ ] Clicking a column header sets cursor to `(colIdx, -1)`
- [ ] Clicking empty board area clears card focus (keeps column)
- [ ] After clicking, keyboard navigation continues from clicked position
- [ ] Double-clicking a card opens inspector

## Tests

- [ ] `kanban-app/ui/src/components/__tests__/board-view.test.tsx` — test click sets cursor
- [ ] `pnpm vitest run` passes