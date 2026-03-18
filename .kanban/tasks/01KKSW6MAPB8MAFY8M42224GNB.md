---
assignees:
- claude-code
depends_on:
- 01KKSW6A68Q9QGZZ3TMR32QXKX
position_column: done
position_ordinal: fffffff880
title: Update frontend BoardProgress to read percent_complete from board entity
---
## What
Update the frontend to read the computed `percent_complete` field from the board entity instead of guessing from column positions.

### Steps

1. **`kanban-app/ui/src/types/kanban.ts`** — Add `done_tasks` and `percent_complete` to `BoardSummary` interface.

2. **`kanban-app/ui/src/components/board-progress.tsx`** — Replace the `useMemo` that guesses done/total from the last column with direct reads from `board.summary.done_tasks` and `board.summary.percent_complete`. Remove the `getNum` import and column-based calculation.

## Acceptance Criteria
- [ ] `BoardSummary` TS type includes `done_tasks` and `percent_complete`
- [ ] `BoardProgress` reads directly from `board.summary`
- [ ] No column-position guessing in frontend
- [ ] Radial chart shows correct percentage
- [ ] `npx tsc --noEmit` passes

## Tests
- [ ] TypeScript compiles cleanly
- [ ] Manual: board with tasks in various columns shows correct percentage