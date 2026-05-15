---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffff8880
title: Replace title= with shadcn Tooltip in nav-bar.tsx
---
## What

`kanban-app/ui/src/components/nav-bar.tsx` uses HTML `title=` attribute on two buttons instead of the shadcn `Tooltip` component:

- Line 49: `title="Inspect board"` on the info button
- Line 67: `title="Search"` on the search button

Replace both with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`. The component is already inside a `TooltipProvider` (provided by WindowContainer/App).

**Files to modify:**
- `kanban-app/ui/src/components/nav-bar.tsx` — import Tooltip components, wrap both buttons

## Acceptance Criteria
- [ ] Both buttons show styled Radix tooltips on hover (not browser-native title tooltips)
- [ ] Tooltip text matches the original title text
- [ ] No HTML `title=` attributes remain in nav-bar.tsx

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over inspect and search buttons, verify styled tooltip appears #tooltip