---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8d80
title: Replace title= with shadcn Tooltip in avatar.tsx
---
## What

`kanban-app/ui/src/components/avatar.tsx` uses HTML `title=` attribute on both avatar variants:

- Line 63: `title={name}` on the `<img>` element (avatar image)
- Line 68: `title={name}` on the `<span>` element (initials fallback)

Replace both with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`. Since avatars render inline in lists (assignee badges), use `side="top"` and a short delay.

**Files to modify:**
- `kanban-app/ui/src/components/avatar.tsx` — import Tooltip components, wrap both img and span variants

## Acceptance Criteria
- [ ] Hovering over an avatar (image or initials) shows a styled Radix tooltip with the actor name
- [ ] No HTML `title=` attributes remain in avatar.tsx
- [ ] Avatar layout is not disrupted by the Tooltip wrapper

## Tests
- [ ] Existing `avatar.test.tsx` still passes (may need selector updates)
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over an assignee avatar on a task card, verify styled tooltip shows the name #tooltip