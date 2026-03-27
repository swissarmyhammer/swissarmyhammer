---
assignees:
- claude-code
depends_on:
- 01KMQMTAMHZHA79PTZAB453KYT
position_column: todo
position_ordinal: c780
title: Migrate board navigation to claimWhen
---
## What

Replace the board's push-based cursor (`useBoardNav` + `board.move*` commands + FocusClaim) with pull-based `claimWhen` on each card and column header FocusScope.

### How it works

Each task card FocusScope (in entity-card or sortable-task-card) declares:\n- `nav.down` claims when the card above me in my column is focused\n- `nav.up` claims when the card below me in my column is focused\n- `nav.left` claims when the same-row card in the column to my right is focused\n- `nav.right` claims when the same-row card in the column to my left is focused

Column headers declare similar predicates for header-to-header and header-to-card transitions.

### Complexity note

This is the most complex migration because:\n- Board layout is 2D (columns × cards) — predicates need row/column awareness\n- Card counts differ per column — moving left/right must clamp card index to the target column's count\n- Column headers (card index -1) participate in the navigation\n- Drag-and-drop reordering changes the card list dynamically\n\nThe ColumnView component already receives the task list and knows ordering. It can compute prev/next monikers and pass `claimWhen` to each card.

### Files to modify

- **`kanban-app/ui/src/components/column-view.tsx`** — compute claimWhen for each card and the column header\n- **`kanban-app/ui/src/components/board-view.tsx`** — remove `board.move*` commands, remove FocusClaim, remove `useBoardNav` cursor state. Keep `board.inspect`, `board.newTask` (action commands, not navigation).\n- **`kanban-app/ui/src/hooks/use-board-nav.ts`** — remove or simplify to mode-only (normal/edit)\n- **`kanban-app/ui/src/components/entity-card.tsx`** or **`sortable-task-card.tsx`** — accept claimWhen and pass to FocusScope

### Cross-column navigation

For `nav.left`/`nav.right`, the card in column B needs to know which card in column A is focused. This requires column-to-column awareness. BoardView can pass adjacent column task lists to each ColumnView, or compute the full claimWhen table at the board level.

## Acceptance Criteria

- [ ] j/k moves between cards within a column via claimWhen\n- [ ] h/l moves between columns, clamping card index to target column's count\n- [ ] Column headers participate in navigation (card -1 position)\n- [ ] `g g`/Home jumps to first card, G/End to last\n- [ ] No `useBoardNav` cursor state — navigation is purely claim-based\n- [ ] FocusClaim removed from board-view\n- [ ] `pnpm vitest run` passes

## Tests

- [ ] `board-view.test.tsx` — nav.down from card 0 focuses card 1 in same column\n- [ ] `board-view.test.tsx` — nav.right from col 0 focuses same-row card in col 1\n- [ ] `board-view.test.tsx` — nav.up from card 0 focuses column header\n- [ ] `board-view.test.tsx` — cross-column clamp: nav.right to shorter column clamps card index\n- [ ] `pnpm vitest run` passes"