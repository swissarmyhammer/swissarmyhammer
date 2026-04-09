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

1. **`kanban-app/ui/src/components/column-view.tsx`** — Delete `nameFieldClaimWhen` (~60 lines), `cardClaimPredicates` (~80 lines), remove `claimWhen` props, remove neighbor-moniker props.
2. **`kanban-app/ui/src/components/board-view.tsx`** — Remove moniker-passing plumbing to columns.
3. **`kanban-app/ui/src/components/sortable-task-card.tsx`** — Remove `claimWhen` prop.
4. **`kanban-app/ui/src/components/entity-card.tsx`** — Remove `claimWhen` prop passthrough.

### Subtasks
- [ ] Delete `nameFieldClaimWhen` and `cardClaimPredicates` memos from column-view.tsx
- [ ] Remove moniker-passing plumbing from board-view.tsx → column-view.tsx props
- [ ] Remove `claimWhen` prop from sortable-task-card.tsx and entity-card.tsx
- [ ] Verify cross-column clamping via Rust test suite (card 2 covers this)
- [ ] Run existing React tests — they should pass with predicates removed

## Acceptance Criteria
- [ ] Column-view has no `claimWhen` predicates — ~140 lines removed
- [ ] Board-view no longer passes moniker arrays to columns
- [ ] All existing column-view and board-view React tests pass unchanged
- [ ] `pnpm vitest run` passes

## Tests

**These are regression tests — the algorithm is tested in Rust (card 2). React tests verify components still render and respond to focus events correctly after predicate removal.**

```
test: "column-view renders cards without claimWhen prop" (column-view.test.tsx)
  setup: render ColumnView with tasks, mock Tauri invokes
  assert: each task card renders, no claimWhen prop errors
  assert: FocusScope elements have data-moniker attributes

test: "board-view renders columns without moniker-passing props" (board-view.test.tsx)
  setup: render BoardView with columns and tasks
  assert: renders without error
  assert: ColumnView does NOT receive leftColumnTaskMonikers, rightColumnTaskMonikers, etc.

test: "entity-card accepts no claimWhen prop" (entity-card.test.tsx)
  setup: render EntityCard without claimWhen
  assert: renders, click invokes spatial_focus
```

**No "manual smoke test" — the Rust test suite in card 2 covers all board layout navigation scenarios with exact rect coordinates. If those pass, the behavior is identical.**

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.