---
assignees:
- claude-code
position_column: todo
position_ordinal: c080
title: Replace title= with shadcn Tooltip in perspective-tab-bar.tsx
---
## What

`kanban-app/ui/src/components/perspective-tab-bar.tsx` uses HTML `title=` attribute on the add-perspective button:

- Line 152: `title="New perspective"`

Replace with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`. The component already imports from command-scope and Radix Popover, so adding Tooltip imports is consistent.

**Files to modify:**
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — import Tooltip components, wrap the add button

## Acceptance Criteria
- [ ] Add-perspective button shows styled Radix tooltip on hover
- [ ] No HTML `title=` attributes remain in perspective-tab-bar.tsx

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over the + button in the perspective tab bar, verify styled tooltip #tooltip