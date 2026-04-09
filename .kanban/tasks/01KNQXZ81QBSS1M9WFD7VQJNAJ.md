---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a480
project: spatial-nav
title: Remove manual claimWhen predicates from column-view and board-view
---
## What

Delete the manual `claimWhen` predicate construction in the board view. Spatial navigation now handles all cardinal direction movement automatically based on DOM position.

### Files to modify

1. **`kanban-app/ui/src/components/column-view.tsx`**:
   - Delete `nameFieldClaimWhen` memo (~60 lines) — the column header FocusScope's rect is naturally above the card FocusScopes, so spatial nav handles up/down/left/right correctly
   - Delete `cardClaimPredicates` memo (~80 lines) — same: cards are vertically stacked, spatial nav handles the rest
   - Remove `claimWhen` props from `<FocusScope>` and `<DraggableTaskCard>` JSX
   - Remove props that only existed for predicate construction: `leftColumnTaskMonikers`, `rightColumnTaskMonikers`, `leftColumnHeaderMoniker`, `rightColumnHeaderMoniker`, `allBoardTaskMonikers`, `allBoardHeaderMonikers`, `isFirstColumn`, `isLastColumn`
   - Remove `ClaimPredicate` import

2. **`kanban-app/ui/src/components/board-view.tsx`**:
   - Remove the moniker-passing plumbing that feeds column-view's predicate construction (the left/right column moniker arrays, allBoardTaskMonikers set, etc.)
   - This is a significant simplification of BoardView's render logic

3. **`kanban-app/ui/src/components/sortable-task-card.tsx`**:
   - Remove `claimWhen` prop — no longer needed
   - Remove `ClaimPredicate` import

4. **`kanban-app/ui/src/components/entity-card.tsx`**:
   - Remove `claimWhen` prop passthrough to FocusScope — no longer needed

### Cross-column navigation behavior to preserve

Today's predicates do explicit clamping: nav.right from column 0 card at index 7 to column 1 with only 3 cards lands on column 1's last card (index 2). Spatial nav preserves this naturally — the nearest rect to the right is the bottom card in the shorter column. But this must be explicitly verified.

Empty column: nav.right from column 0's last card to an empty column 1 should land on column 1's header (the only FocusScope in that column). Spatial nav handles this — the header rect is the only candidate to the right in that column's X range.

### Subtasks
- [ ] Delete `nameFieldClaimWhen` and `cardClaimPredicates` memos from column-view.tsx
- [ ] Remove moniker-passing plumbing from board-view.tsx → column-view.tsx props
- [ ] Remove `claimWhen` prop from sortable-task-card.tsx and entity-card.tsx
- [ ] Verify cross-column clamping: nav.right from tall column to short column lands on nearest card
- [ ] Verify empty column: nav.right to empty column lands on its header

## Acceptance Criteria
- [ ] Column-view has no `claimWhen` predicates — ~140 lines of predicate code removed
- [ ] Board-view no longer passes moniker arrays to columns
- [ ] `nav.up`/`nav.down` moves between cards in a column via spatial proximity
- [ ] `nav.left`/`nav.right` moves between columns via spatial proximity
- [ ] `nav.first`/`nav.last` finds top-left / bottom-right card on the board
- [ ] Cross-column clamping works: nav.right from card 7 in 10-card column to 3-card column focuses card 3 (nearest)
- [ ] Empty column: nav.right lands on column header when column has no cards
- [ ] Right from last column = no-op (no candidate to the right)
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `Rust unit tests` — 3-column board layout: verify cross-column nav, clamping, and empty column scenarios (these are tested in Rust since that's where navigate() lives)
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — column navigation still works (update tests to not assert on predicates)
- [ ] `kanban-app/ui/src/components/board-view.test.tsx` — cross-column navigation still works
- [ ] Manual smoke test: board view keyboard navigation feels identical
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.