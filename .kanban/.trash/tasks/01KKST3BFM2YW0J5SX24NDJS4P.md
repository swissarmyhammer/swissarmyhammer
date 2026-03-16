---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8180'
title: Update frontend BoardSummary type and BoardProgress to use backend percent_complete
---
## What
Update the frontend to consume the new `done_tasks` and `percent_complete` fields from the backend instead of guessing from the last column's `task_count`.

Files to change:
1. **`kanban-app/ui/src/types/kanban.ts`** — Add `done_tasks: number` and `percent_complete: number` to `BoardSummary` interface
2. **`kanban-app/ui/src/components/board-progress.tsx`** — Replace the `useMemo` that computes `done`/`total`/`pct` from column data with direct reads from `board.summary.done_tasks` and `board.summary.percent_complete`

## Acceptance Criteria
- [ ] `BoardSummary` TS type includes `done_tasks` and `percent_complete`
- [ ] `BoardProgress` reads `percent_complete` directly from `board.summary`
- [ ] No more guessing based on last column position
- [ ] Radial chart shows correct percentage
- [ ] Tooltip shows correct done/total count

## Tests
- [ ] `npx tsc --noEmit` passes
- [ ] Manual: verify radial chart shows correct percentage with tasks in various columns