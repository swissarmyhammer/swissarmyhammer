---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: ff9380
project: spatial-nav
title: 'Column: wrap as zone, strip legacy keyboard nav from column-view'
---
## What

Wrap each column in `<FocusZone moniker="column:{id}">` and strip every legacy keyboard-nav vestige from `column-view.tsx`. The column zone sits inside the board zone (parent zone = `ui:board`) and contains the column header (a leaf) plus task cards (each its own zone, separate card).

### Files to modify

- `kanban-app/ui/src/components/column-view.tsx`

### Zone shape (parent context)

```
ui:board (FocusZone)
  column:{id} (FocusZone) ← THIS CARD
    column_header (Leaf)
    task:{id} (FocusZone) ← separate "card as zone" card
```

### Legacy nav to remove from column-view.tsx

- `nameFieldClaimWhen` memo construction (~60 lines)
- `cardClaimPredicates` memo (~80 lines)
- `cellMonikerMap` if only used by predicates
- `claimWhen` props passed down to cards / column headers
- Neighbor-moniker props received from board-view.tsx
- `ClaimPredicate` import
- Any `onKeyDown` handlers on the column div
- Any column-scoped `keydown` listeners

What stays: column-collapse toggles via click, drop-target hooks for drag-drop.

### Subtasks
- [ ] Wrap column body in `<FocusZone moniker={Moniker(`column:${columnId}`)}>`
- [ ] Column header inside the zone stays as a `<Focusable>` or `<FocusScope>` (default leaf)
- [ ] Delete `nameFieldClaimWhen` and `cardClaimPredicates` memos
- [ ] Delete `cellMonikerMap` (only used by predicates)
- [ ] Remove neighbor-moniker props from ColumnView's interface
- [ ] Remove `ClaimPredicate` import
- [ ] Remove any column-level keyboard listeners

## Acceptance Criteria
- [ ] Each column registers as a `FocusZone` with moniker `column:{id}` and `parent_zone = ui:board`
- [ ] Column header registers as a leaf with `parent_zone = column:{id}`
- [ ] Card zones (from "card as zone" card) appear with `parent_zone = column:{id}`
- [ ] ~140 lines of predicate code removed from column-view.tsx
- [ ] No `ClaimPredicate` import; no `claimWhen` prop on any child
- [ ] No `onKeyDown` / `keydown` listener at the column level
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `column-view.test.tsx` — column registers as a Zone, parent_zone is the board zone
- [ ] `column-view.test.tsx` — column header registers as a Leaf inside the column zone
- [ ] `column-view.test.tsx` — no `claimWhen` props on children, no neighbor-moniker props received
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.