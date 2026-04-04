---
assignees:
- claude-code
position_column: todo
position_ordinal: c180
title: Replace title= with shadcn Tooltip in board-selector.tsx
---
## What

`kanban-app/ui/src/components/board-selector.tsx` uses HTML `title=` attribute on the tear-off button:

- Line 131: `title="Open in new window"`

Replace with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`.

**Files to modify:**
- `kanban-app/ui/src/components/board-selector.tsx` — import Tooltip components, wrap the tear-off button

## Acceptance Criteria
- [ ] Tear-off button shows styled Radix tooltip on hover
- [ ] No HTML `title=` attributes remain in board-selector.tsx

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over the new-window icon in the board selector, verify styled tooltip #tooltip