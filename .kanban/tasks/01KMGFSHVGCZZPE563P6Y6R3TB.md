---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffd80
title: useBoardNav hook — 2D cursor state for board navigation
---
## What

Create `kanban-app/ui/src/hooks/use-board-nav.ts` — a navigation hook for the board view, following the same patterns as `use-grid.ts` and `use-inspector-nav.ts`.

The board is a 2D space: columns (horizontal) × cards within a column (vertical). The cursor tracks `{ col: number, card: number }`. Unlike the grid which has uniform row counts, each column has a different number of cards, so vertical clamping must use per-column card counts.

### Cursor model

```typescript
export type BoardMode = \"normal\" | \"edit\";

export interface BoardCursor {
  col: number;   // column index
  card: number;  // card index within column (-1 means column header focused)
}
```

When moving left/right between columns, the card index should be clamped to the new column's card count (preserve approximate vertical position). When a column is empty, card stays at -1 (column-level focus).

### API surface

```typescript
export interface UseBoardNavOptions {
  columnCount: number;
  cardCounts: number[];  // length === columnCount
}

export interface UseBoardNavReturn {
  cursor: BoardCursor;
  mode: BoardMode;
  moveLeft: () => void;
  moveRight: () => void;
  moveUp: () => void;
  moveDown: () => void;
  moveToFirstColumn: () => void;
  moveToLastColumn: () => void;
  moveToFirstCard: () => void;
  moveToLastCard: () => void;
  setCursor: (col: number, card: number) => void;
  enterEdit: () => void;
  exitEdit: () => void;
}
```

### Files to create/modify

- **Create**: `kanban-app/ui/src/hooks/use-board-nav.ts`
- **Create**: `kanban-app/ui/src/hooks/__tests__/use-board-nav.test.ts`

## Acceptance Criteria

- [ ] Hook tracks `{ col, card }` cursor clamped to column/card bounds
- [ ] `moveLeft`/`moveRight` change column, clamping card to new column's count
- [ ] `moveUp`/`moveDown` navigate cards within current column
- [ ] `moveUp` from card 0 goes to card -1 (column header)
- [ ] `moveDown` from card -1 goes to card 0
- [ ] Empty column: card stays at -1, moveDown is a no-op
- [ ] `enterEdit`/`exitEdit` toggle BoardMode
- [ ] Hook is pure state — no DOM interaction, no side effects

## Tests

- [ ] `kanban-app/ui/src/hooks/__tests__/use-board-nav.test.ts` — unit tests using `renderHook`
- [ ] Test: move right clamps card to shorter column
- [ ] Test: move down in empty column is no-op
- [ ] Test: move up from card 0 → card -1 (header)
- [ ] `pnpm vitest run` passes</description>
</invoke>