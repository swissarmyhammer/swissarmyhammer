---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a480
project: spatial-nav
title: 'Board view: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the **board view container** in `<FocusZone moniker="ui:board">` and strip every legacy keyboard-nav vestige from `board-view.tsx`. The board zone is the root of the column/card subtree on the kanban view; columns and cards each become zones in their own dedicated cards (column zone, card zone).

### Files to modify

- `kanban-app/ui/src/components/board-view.tsx`

### Zone shape

```
window root layer
  ...other shell zones (navbar, toolbar, perspective)...
  ui:board (FocusZone) ← THIS CARD
    column:{id} (FocusZone) ← separate "column as zone" card
      column header (Leaf)
      task:{id} (FocusZone) ← separate "card as zone" card
        title (Leaf), status (Leaf), pills (Leaves)
```

### Legacy nav to remove from board-view.tsx

Sweep the file for everything that bypasses spatial nav:
- The `cardClaimPredicates` / `nameFieldClaimWhen` memo construction (these were in board-view.tsx for cross-column moniker plumbing)
- The neighbor-moniker arrays (`leftColumnTaskMonikers`, `rightColumnTaskMonikers`, `aboveTaskMonikers`, etc.) — they exist solely to feed claimWhen predicates
- Any `onKeyDown` handlers wired to the outer board div (if any)
- Any `useEffect` listening for `keydown` at the document level scoped to the board view
- `ClaimPredicate` import and threading

What stays: drag-drop handlers (DnD-Kit `useDndMonitor` etc.) — those are unrelated to keyboard nav.

### Subtasks
- [ ] Wrap board content in `<FocusZone moniker={Moniker("ui:board")}>` (the moniker is non-entity chrome — stable per window)
- [ ] Delete `cardClaimPredicates` / `nameFieldClaimWhen` memos from board-view.tsx
- [ ] Delete neighbor-moniker prop plumbing into ColumnView
- [ ] Remove `ClaimPredicate` import
- [ ] Remove any board-level keyboard listeners that are now redundant

## Acceptance Criteria
- [ ] Board view registers exactly one Zone (`ui:board`) at its root; columns appear as children in `parent_zone`
- [ ] Zero predicate / neighbor-moniker plumbing remains in board-view.tsx
- [ ] No `ClaimPredicate` import in board-view.tsx
- [ ] No `onKeyDown` / `keydown` listener in board-view.tsx (other than DnD-Kit's own internals)
- [ ] All existing board-view tests pass after changes
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `board-view.test.tsx` — board container registers as a Zone with `parent_zone = window_root`
- [ ] `board-view.test.tsx` — no `claimWhen` / `ClaimPredicate` props accepted
- [ ] `board-view.test.tsx` — no neighbor-moniker props (`leftColumnTaskMonikers`, etc.)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.