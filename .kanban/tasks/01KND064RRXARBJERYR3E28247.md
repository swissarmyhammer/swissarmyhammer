---
assignees:
- claude-code
position_column: todo
position_ordinal: bf80
title: Replace title= with shadcn Tooltip in column-view.tsx
---
## What

`kanban-app/ui/src/components/column-view.tsx` uses HTML `title=` attribute on the add-task button:

- Line 478: `` title={`Add task to ${getStr(column, "name")}`} ``

Replace with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`.

**Files to modify:**
- `kanban-app/ui/src/components/column-view.tsx` — import Tooltip components, wrap the add-task button

## Acceptance Criteria
- [ ] Add-task button shows styled Radix tooltip on hover with the column name
- [ ] No HTML `title=` attributes remain in column-view.tsx

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over the + button in a column header, verify styled tooltip shows "Add task to {column name}" #tooltip