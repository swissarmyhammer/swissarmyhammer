---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8480
title: Wire GroupedBoardView into ViewContainer and fix BoardView groupValue prop
---
## What

`GroupedBoardView`, `GroupSection`, and `computeGroups` were all implemented but never connected. The view container still renders `BoardView` directly, so grouping only re-sorts cards within columns instead of showing a vertical stack of collapsible group sections each containing a full board.

Two problems to fix:

### 1. `view-container.tsx` renders `BoardView` instead of `GroupedBoardView`

In `kanban-app/ui/src/components/view-container.tsx`, `ActiveViewRenderer` (around line 86-87) renders:
```tsx
return <BoardView board={board} tasks={tasks} />;
```
This must become:
```tsx
return <GroupedBoardView board={board} tasks={tasks} />;
```
`GroupedBoardView` already handles the no-group case by delegating to `BoardView` internally, so this is a safe swap.

### 2. `BoardView` doesn't accept `groupValue` prop

`GroupSection` (line 59) passes `groupValue={bucket.value}` to `BoardView`, but `BoardViewProps` (in `kanban-app/ui/src/components/board-view.tsx` around line 39-42) only has `board` and `tasks`. Add `groupValue?: string` to `BoardViewProps` so TypeScript compiles and the value is available for drop zone plumbing.

Additionally, `BoardView` still has the inline group-sort logic (lines 111-115 in `baseLayout`) that clusters cards by group value. When rendered inside `GroupedBoardView`, each `BoardView` already receives only that group's tasks, so the group sort is redundant. The sort should be skipped when `groupValue` is provided (meaning we're inside a group section — tasks are already filtered to one group).

### Files to modify

1. **`kanban-app/ui/src/components/view-container.tsx`**:
   - Change import from `BoardView` to `GroupedBoardView`
   - Replace `<BoardView board={board} tasks={tasks} />` with `<GroupedBoardView board={board} tasks={tasks} />`

2. **`kanban-app/ui/src/components/board-view.tsx`**:
   - Add `groupValue?: string` to `BoardViewProps`
   - Accept and destructure `groupValue` in `BoardView`
   - In `baseLayout` useMemo: skip the `groupField` sort when `groupValue` is defined (tasks are pre-filtered to one group)

## Acceptance Criteria

- [ ] Selecting a group field in the perspective shows a vertical stack of collapsible sections
- [ ] Each section header shows the group label, task count badge, and collapse chevron
- [ ] Each expanded section contains a full horizontal column layout for that group's tasks
- [ ] Clicking a section header collapses/expands it
- [ ] With no grouping active, the board looks exactly as before
- [ ] TypeScript compiles without errors (groupValue prop accepted)
- [ ] `npm test` passes

## Tests

- [ ] `kanban-app/ui/src/components/view-container.tsx` — verify `GroupedBoardView` is rendered for board views (update existing test if needed)
- [ ] `kanban-app/ui/src/components/grouped-board-view.test.tsx` — existing tests should now pass if they were skipped
- [ ] `npm test` — full suite passes
- [ ] `npx tsc --noEmit` — no type errors

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.">
