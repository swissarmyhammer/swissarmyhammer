---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a480
project: spatial-nav
title: 'Board view: wrap columns and cards as zones, delete claimWhen predicates'
---
## What

Migrate the board view to the spatial-nav zone model, then delete the manual `claimWhen` predicate machinery. Columns and task cards both become Zones; card internals (title, status, pills) are Leaves. The three-rule beam search handles cross-column nav naturally.

### Zone hierarchy in a board

```
window root layer
  board content area (parent zone — the ViewContainer)
    col0 (Zone, parent_zone = board)
      column_header (Leaf, parent_zone = col0)
      card_0 (Zone, parent_zone = col0)
        card_title (Leaf, parent_zone = card_0)
        card_status (Leaf, parent_zone = card_0)
        card_assignee_pill (Leaf, parent_zone = card_0)
        ...
      card_1 (Zone, parent_zone = col0)
        ...
    col1 (Zone, parent_zone = board)
      ...
```

### How nav behaves with this shape

- Focused on `card_0.title`: arrow down → `card_0.status` (rule 1: in-zone). Arrow down past the last leaf in the card → rule 2 fallback → `card_1.title` (nearest leaf below in same layer, crossing the card zone boundary).
- Arrow right from `card_0.title`: rule 1 empty (no sibling right in the card) → rule 2 → `col1.card_k.title` (aligned across columns).
- Focused on `col0` (zone level, reached via drill-out): arrow right → `col1`. Leaves invisible at zone level.
- Column header is a Leaf (peer to the cards visually, though structurally it sits in the column zone alongside cards).

### Files to modify

1. **`kanban-app/ui/src/components/column-view.tsx`**
   - Wrap the column body in `<FocusScope kind="zone" moniker={`column:${columnId}`}>`
   - Delete `nameFieldClaimWhen` and `cardClaimPredicates` memos
   - Delete `cellMonikerMap` / neighbor-moniker threading
   - Remove `claimWhen` props passed down
   - Remove `ClaimPredicate` import
   - Column header is a Leaf — just a `<FocusScope>` with default `kind`.

2. **`kanban-app/ui/src/components/board-view.tsx`**
   - Remove the neighbor-moniker plumbing (`leftColumnTaskMonikers`, etc.) passed into `ColumnView`.

3. **`kanban-app/ui/src/components/sortable-task-card.tsx`** and **`entity-card.tsx`**
   - Wrap the card in `<FocusScope kind="zone" moniker={cardMoniker}>`.
   - Inner title, status, pills are `<FocusScope kind="leaf">` (default) — most likely already set up that way; verify.
   - Remove `claimWhen` prop passthrough and import.

### Subtasks
- [ ] Add `<FocusScope kind="zone">` wrapping in column-view.tsx
- [ ] Add `<FocusScope kind="zone">` wrapping for cards in sortable-task-card.tsx / entity-card.tsx
- [ ] Delete `nameFieldClaimWhen` and `cardClaimPredicates` memos
- [ ] Remove neighbor-moniker plumbing from board-view.tsx → column-view.tsx props
- [ ] Remove `claimWhen` prop from sortable-task-card.tsx / entity-card.tsx

## Acceptance Criteria
- [ ] Column-view, card, and card internals reflect the zone hierarchy above
- [ ] Column-view has no `claimWhen` predicates (roughly 140 lines removed)
- [ ] Board-view no longer passes moniker arrays to columns
- [ ] Cross-column arrow nav works via beam rule 2 (cross-zone leaf fallback)
- [ ] Within-card arrow nav works via beam rule 1 (within-zone)
- [ ] Drill-out onto a column zone + arrow-right goes to next column (beam sibling zones)
- [ ] All existing column-view and board-view React tests pass after predicate removal
- [ ] `pnpm vitest run` passes

## Tests

Algorithm correctness is covered in Rust (card `01KNQXXF5W7G4JP73C6ZCMKYKX`). React tests here verify the component tree emits the right FocusScope shape.

- [ ] `column-view.test.tsx` — ColumnView mount registers one Zone entry (the column) plus leaf entries for header + per-card zones; no `claimWhen` prop errors
- [ ] `column-view.test.tsx` — no more moniker-neighbor props accepted
- [ ] `board-view.test.tsx` — renders without `leftColumnTaskMonikers` / `rightColumnTaskMonikers` props
- [ ] `entity-card.test.tsx` — card wrapper is `kind="zone"`, no `claimWhen` prop
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.