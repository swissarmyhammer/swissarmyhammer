---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffb080
title: 'Bug: GridView crashes — activePerspective not destructured from useActivePerspective()'
---
## What

`grid-view.tsx` references `activePerspective` on lines 492–493 but never destructures it from `useActivePerspective()`. Line 37 only extracts `{ applyFilter, applySort, groupField }`, leaving `activePerspective` as an undeclared variable. This causes a `ReferenceError` and white screen when switching to grid view.

**Root cause**: Missing destructured field — simple one-line fix.

**Secondary concern (user's question)**: `activePerspective` can be `null` when no perspectives exist (see `perspective-context.tsx:128-132`). The `?.` optional chaining on lines 492-493 handles null gracefully (passes `undefined` to DataTable which treats both props as optional). However, `PerspectiveContainer` (`perspective-container.tsx:85-114`) also passes `activePerspective` through and it CAN be null. The current code is safe because DataTable's `perspectiveSort` and `perspectiveId` props are already typed as optional. No default-perspective upsert is needed to fix this crash — the crash is purely the missing destructure.

### Files to modify

- `kanban-app/ui/src/components/grid-view.tsx` — line 37: add `activePerspective` to the destructure

## Acceptance Criteria

- [ ] Switching to grid view no longer throws `ReferenceError: Can't find variable: activePerspective`
- [ ] Grid view renders correctly when `activePerspective` is null (no perspectives exist)
- [ ] Grid view renders correctly when `activePerspective` has sort entries
- [ ] Existing grid-view tests pass

## Tests

- [ ] Add test in `kanban-app/ui/src/components/grid-view.test.tsx`: render GridView with a mock perspective that has sort entries, verify `perspectiveSort` and `perspectiveId` are passed to DataTable
- [ ] Add test: render GridView with `activePerspective: null`, verify no crash and DataTable receives `undefined` for both props
- [ ] Run: `cd kanban-app/ui && npx vitest run grid-view` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.